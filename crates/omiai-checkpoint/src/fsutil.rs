//! Filesystem primitives: BLAKE3 hashing and crash-safe atomic writes.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::CheckpointError;

/// Hash a file's contents with BLAKE3, returned as lowercase hex.
pub fn hash_file(path: &Path) -> Result<String, CheckpointError> {
    let mut hasher = blake3::Hasher::new();
    let mut file = fs::File::open(path).map_err(|source| CheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    std::io::copy(&mut file, &mut hasher).map_err(|source| CheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// Read a file's contents into a byte vector.
pub fn read_file(path: &Path) -> Result<Vec<u8>, CheckpointError> {
    fs::read(path).map_err(|source| CheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Write `bytes` to `dir/name` atomically: write a hidden temp sibling,
/// flush it to disk, rename over the target, then fsync the directory so
/// the rename itself is durable. On success no `.tmp` residue remains.
pub fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), CheckpointError> {
    let target = dir.join(name);
    let tmp = dir.join(format!(".{name}.tmp"));

    {
        let mut f = fs::File::create(&tmp).map_err(|source| CheckpointError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.write_all(bytes).map_err(|source| CheckpointError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.sync_all().map_err(|source| CheckpointError::Io {
            path: tmp.clone(),
            source,
        })?;
    }

    fs::rename(&tmp, &target).map_err(|source| CheckpointError::Io {
        path: target.clone(),
        source,
    })?;

    // fsync the directory entry so the rename survives power loss (unix).
    #[cfg(unix)]
    {
        let d = fs::File::open(dir).map_err(|source| CheckpointError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        d.sync_all().map_err(|source| CheckpointError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}
