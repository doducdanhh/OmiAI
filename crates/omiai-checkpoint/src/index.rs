//! Checkpoint index: maps logical names/steps to checkpoint directories.
//! The sliding-window retention policy (keep N most recent + milestones
//! every K steps) is layered on top of this in a later slice; for now the
//! index is the discovery surface for `step_*` directories.

use std::path::{Path, PathBuf};

use crate::error::CheckpointError;

/// Discover checkpoint step-directories under `root`, sorted ascending by
/// their `step_XXXXXXXX` number.
pub fn list_steps(root: &Path) -> Result<Vec<(u64, PathBuf)>, CheckpointError> {
    let mut steps = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|source| CheckpointError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CheckpointError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(num) = name
            .strip_prefix("step_")
            .and_then(|s| s.parse::<u64>().ok())
        {
            steps.push((num, entry.path()));
        }
    }
    steps.sort_by_key(|(n, _)| *n);
    Ok(steps)
}
