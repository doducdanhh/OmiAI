//! Durable, versioned persistence for long-running OmiAI state.
//!
//! Checkpoints are written to a sibling temporary file, flushed to stable
//! storage, and then promoted. The previous valid state is retained as a
//! backup so an interrupted replacement can be recovered.

use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current checkpoint envelope format.
pub const STATE_FORMAT_VERSION: u32 = 1;

/// Errors produced by durable state operations.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("state serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported state format {found}; maximum supported is {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("state checksum mismatch")]
    ChecksumMismatch,
    #[error("state path has no parent directory")]
    MissingParent,
}

/// Versioned state wrapper with an integrity checksum over its payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEnvelope<T> {
    pub format_version: u32,
    pub checksum: u64,
    pub payload: T,
}

impl<T: Serialize> StateEnvelope<T> {
    /// Wrap a payload and calculate its deterministic FNV-1a checksum.
    pub fn new(payload: T) -> Result<Self, PersistenceError> {
        let checksum = payload_checksum(&payload)?;
        Ok(Self {
            format_version: STATE_FORMAT_VERSION,
            checksum,
            payload,
        })
    }

    /// Verify version compatibility and payload integrity.
    pub fn verify(&self) -> Result<(), PersistenceError> {
        if self.format_version > STATE_FORMAT_VERSION {
            return Err(PersistenceError::UnsupportedVersion {
                found: self.format_version,
                supported: STATE_FORMAT_VERSION,
            });
        }
        if payload_checksum(&self.payload)? != self.checksum {
            return Err(PersistenceError::ChecksumMismatch);
        }
        Ok(())
    }
}

/// Atomically checkpoint serializable state while retaining one backup.
pub fn save_checkpoint<T: Serialize>(
    path: impl AsRef<Path>,
    state: &T,
) -> Result<(), PersistenceError> {
    let path = path.as_ref();
    let parent = path.parent().ok_or(PersistenceError::MissingParent)?;
    fs::create_dir_all(parent)?;

    let envelope = StateEnvelope::new(state)?;
    let temporary = sibling_path(path, "tmp");
    let backup = sibling_path(path, "bak");

    {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &envelope)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }

    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(PersistenceError::Io(error));
    }

    sync_directory(parent)?;
    Ok(())
}

/// Load and verify a checkpoint, falling back to its backup if needed.
pub fn load_checkpoint<T: DeserializeOwned + Serialize>(
    path: impl AsRef<Path>,
) -> Result<T, PersistenceError> {
    let path = path.as_ref();
    match load_envelope(path) {
        Ok(envelope) => Ok(envelope.payload),
        Err(primary_error) => {
            let backup = sibling_path(path, "bak");
            if backup.exists() {
                return load_envelope(&backup).map(|envelope| envelope.payload);
            }
            Err(primary_error)
        }
    }
}

fn load_envelope<T: DeserializeOwned + Serialize>(
    path: &Path,
) -> Result<StateEnvelope<T>, PersistenceError> {
    let envelope: StateEnvelope<T> = serde_json::from_reader(BufReader::new(File::open(path)?))?;
    envelope.verify()?;
    Ok(envelope)
}

fn payload_checksum<T: Serialize>(payload: &T) -> Result<u64, PersistenceError> {
    let bytes = serde_json::to_vec(payload)?;
    Ok(bytes
        .into_iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        }))
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{suffix}"));
    path.with_file_name(name)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("omiai-checkpoint-{}.json", uuid::Uuid::new_v4()));
        save_checkpoint(&path, &vec!["knowledge", "proof"])?;
        let loaded: Vec<String> = load_checkpoint(&path)?;
        assert_eq!(loaded, ["knowledge", "proof"]);
        let _ = fs::remove_file(path);
        Ok(())
    }
}
