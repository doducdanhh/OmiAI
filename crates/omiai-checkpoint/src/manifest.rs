//! checkpoint-v1 manifest: one JSON document per step directory that pins
//! the format version and BLAKE3 hashes of every payload file.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CheckpointError;

/// One hashed file entry inside a manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    /// Path relative to the checkpoint directory (e.g. `grid.bin`).
    pub path: String,
    /// BLAKE3 hex digest of the file contents.
    pub blake3: String,
}

/// The `manifest.json` of a checkpoint-v1 directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Bumped on any incompatible format change; currently always 1.
    pub format_version: u32,
    /// Git commit the producing binary was built from, when known.
    pub git_commit: Option<String>,
    /// Simulation/logical step at which the checkpoint was taken.
    pub step: u64,
    /// RFC 3339 UTC timestamp of creation.
    pub timestamp_utc: String,
    /// Seed of the RNG that produced the state being persisted.
    pub rng_seed: u64,
    /// Opaque serialized RNG stream state, hex-encoded.
    pub rng_state_hex: String,
    /// Hash records for every payload file in the directory.
    pub files: Vec<FileRecord>,
}

pub const MANIFEST_NAME: &str = "manifest.json";
/// Format version this crate reads and writes.
pub const FORMAT_VERSION_V1: u32 = 1;

impl Manifest {
    /// Write `manifest.json` into `dir` via an atomic replace.
    pub fn write(dir: &Path, files: &[FileRecord]) -> Result<(), CheckpointError> {
        let m = Manifest {
            format_version: FORMAT_VERSION_V1,
            git_commit: option_env!("OMIAI_GIT_COMMIT").map(str::to_string),
            step: 0,
            timestamp_utc: String::new(),
            rng_seed: 0,
            rng_state_hex: String::new(),
            files: files.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&m)?;
        super::write_atomic(dir, MANIFEST_NAME, &bytes)
    }

    /// Read and parse `manifest.json` from `dir`.
    pub fn read(dir: &Path) -> Result<Manifest, CheckpointError> {
        let path = dir.join(MANIFEST_NAME);
        let bytes = std::fs::read(&path).map_err(|source| CheckpointError::Io {
            path,
            source,
        })?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
