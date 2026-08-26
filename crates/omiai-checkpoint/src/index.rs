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

// ---------------------------------------------------------------------------
// index.json — atomic write + fallback rebuild
// ---------------------------------------------------------------------------

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::fsutil::write_atomic;

/// Tên file index trong thư mục checkpoints/.
pub const INDEX_NAME: &str = "index.json";

/// Một entry index: step + tên thư mục con tương ứng.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointIndexEntry {
    pub step: u64,
    pub dir: String,
}

/// Index các checkpoint hợp lệ, tăng dần theo step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointIndex {
    pub entries: Vec<CheckpointIndexEntry>,
}

/// Ghi index.json bằng ghi nguyên tử (tmp + rename).
pub fn write_index(
    root: &Path,
    index: &CheckpointIndex,
) -> Result<(), CheckpointError> {
    let mut entries = index.entries.clone();
    entries.sort_by_key(|e| e.step);
    let normalized = CheckpointIndex { entries };
    let bytes =
        serde_json::to_vec_pretty(&normalized).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
    // write_atomic đã trả CheckpointError — trả thẳng.
    write_atomic(root, INDEX_NAME, &bytes)
}

/// Đọc index.json; nếu thiếu/hỏng/thiếu step trên đĩa → quét thư mục rebuild.
///
/// Trả về `(index, rebuilt)`: `rebuilt = true` khi index vừa được dựng lại
/// từ quét thư mục (caller nên log cảnh báo — không bao giờ im lặng tuyệt đối).
pub fn read_or_rebuild_index(
    root: &Path,
) -> Result<(CheckpointIndex, bool), CheckpointError> {
    let on_disk = list_steps(root)?;

    let from_file: Option<CheckpointIndex> = std::fs::read(root.join(INDEX_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());

    if let Some(mut idx) = from_file {
        idx.entries.sort_by_key(|e| e.step);
        let indexed: HashSet<u64> = idx.entries.iter().map(|e| e.step).collect();
        let all_on_disk = on_disk.iter().all(|(s, _)| indexed.contains(s));
        if all_on_disk {
            return Ok((idx, false));
        }
    }

    let entries = on_disk
        .iter()
        .map(|(s, p)| CheckpointIndexEntry {
            step: *s,
            dir: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
        .collect();
    Ok((CheckpointIndex { entries }, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("omiai-idx-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn list_steps_sorted_ascending() {
        let root = temp_root("list");
        for s in [9u64, 1, 5] {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
        let steps = list_steps(&root).unwrap();
        assert_eq!(
            steps.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![1, 5, 9]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_then_read_round_trips() {
        let root = temp_root("rt");
        let idx = CheckpointIndex {
            entries: vec![
                CheckpointIndexEntry { step: 5, dir: "step_00000005".into() },
                CheckpointIndexEntry { step: 3, dir: "step_00000003".into() },
            ],
        };
        write_index(&root, &idx).unwrap();
        let (read, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(!rebuilt);
        // Đã chuẩn hoá tăng dần khi ghi.
        assert_eq!(read.entries[0].step, 3);
        assert_eq!(read.entries[1].step, 5);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_index_falls_back_to_scan() {
        let root = temp_root("miss");
        for s in [1u64, 2, 7] {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 3);
        assert_eq!(idx.entries[2].step, 7);
        assert_eq!(idx.entries[2].dir, "step_00000007");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_index_falls_back_to_scan() {
        let root = temp_root("corrupt");
        std::fs::create_dir_all(root.join("step_00000009")).unwrap();
        std::fs::write(root.join(INDEX_NAME), b"{not json").unwrap();
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stale_index_missing_step_rebuilds() {
        let root = temp_root("stale");
        for s in [1u64, 2] {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
        // Index chỉ ghi step 1, thiếu step 2 đang tồn tại trên đĩa.
        write_index(
            &root,
            &CheckpointIndex {
                entries: vec![CheckpointIndexEntry {
                    step: 1,
                    dir: "step_00000001".into(),
                }],
            },
        )
        .unwrap();
        let (idx, rebuilt) = read_or_rebuild_index(&root).unwrap();
        assert!(rebuilt);
        assert_eq!(idx.entries.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
