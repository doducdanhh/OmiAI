//! Versioned directory-format checkpoints for long-running training/
//! evolution sessions.
//!
//! Design (spec §2, docs/format-spec/checkpoint-v1.md): each checkpoint is
//! a *directory* `step_XXXXXXXX/` with a `manifest.json` (format version,
//! git commit, step, timestamp, full RNG state, per-file BLAKE3 hashes).
//! Writes are atomic: tmp file → fsync → rename → fsync dir. A sliding
//! window keeps the N most recent plus permanent milestones every K steps.
//!
//! This slice ships the [`Checkpointable`] trait, atomic-write/hash
//! helpers, manifest + `verify_dir`, and the first round-trip
//! implementation for the world CA grid.

pub mod ca_grid;
pub mod communication;
pub mod error;
pub mod fsutil;
pub mod index;
pub mod legacy;
pub mod manifest;
pub mod retention;
pub mod traits;
pub mod world_bundle;

pub use error::CheckpointError;
pub use retention::{apply_retention, RetentionPolicy};
pub use fsutil::{hash_file, write_atomic, read_file};
pub use manifest::{FileRecord, Manifest, FORMAT_VERSION_V1};
pub use traits::Checkpointable;
pub use omiai_world::WorldConfig;
pub use world_bundle::{GraphFile, restore_rng, AtomsFile, RegistryFile, KNOWLEDGE_DIR, GRAPH_FILE, COMM_DIR, CONVENTIONS_FILE};

use std::path::Path;

/// Verify a checkpoint directory: read its manifest and re-hash every
/// recorded file, failing on any mismatch or missing payload.
pub fn verify_dir(dir: &Path) -> Result<(), CheckpointError> {
    let manifest = Manifest::read(dir)?;
    if manifest.format_version != manifest::FORMAT_VERSION_V1 {
        return Err(CheckpointError::MissingField(format!(
            "unsupported format_version {}",
            manifest.format_version
        )));
    }
    for record in &manifest.files {
        let path = dir.join(&record.path);
        let actual = hash_file(&path).map_err(|e| match e {
            CheckpointError::Io { source, .. } => CheckpointError::Io {
                path: path.clone(),
                source,
            },
            other => other,
        })?;
        if actual != record.blake3 {
            return Err(CheckpointError::Corrupt {
                path,
                expected: record.blake3.clone(),
                actual,
            });
        }
    }
    Ok(())
}
