//! Sliding-window retention for checkpoint directories (spec gốc mục 2.3).
//!
//! Giữ N step gần nhất CỘNG mọi mốc vĩnh viễn (step chia hết cho
//! `milestone_every`, gồm step 0). Không bao giờ xoá mốc.

use std::path::{Path, PathBuf};

use crate::error::CheckpointError;
use crate::index::list_steps;

/// Chính sách giữ checkpoint: N gần nhất + mốc mỗi K bước.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    pub keep_recent: usize,
    pub milestone_every: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_recent: 10,
            milestone_every: 100,
        }
    }
}

/// Xoá các checkpoint ngoài chính sách; trả về danh sách đã xoá.
pub fn apply_retention(
    root: &Path,
    policy: &RetentionPolicy,
) -> Result<Vec<(u64, PathBuf)>, CheckpointError> {
    let mut steps = list_steps(root)?;
    // Gần nhất trước.
    steps.sort_by_key(|&(s, _)| std::cmp::Reverse(s));

    let mut removed = Vec::new();
    for (i, (step, path)) in steps.iter().enumerate() {
        let is_recent = i < policy.keep_recent;
        let is_milestone =
            policy.milestone_every > 0 && step % policy.milestone_every == 0;
        if !is_recent && !is_milestone {
            std::fs::remove_dir_all(path).map_err(|source| CheckpointError::Io {
                path: path.clone(),
                source,
            })?;
            removed.push((*step, path.clone()));
        }
    }
    removed.sort_by_key(|(s, _)| *s);
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tạo thư mục checkpoint giả `step_XXXXXXXX` rỗng (chỉ cần tên đúng).
    fn make_fake_checkpoints(root: &Path, steps: &[u64]) {
        std::fs::create_dir_all(root).unwrap();
        for s in steps {
            std::fs::create_dir_all(root.join(format!("step_{s:08}"))).unwrap();
        }
    }

    #[test]
    fn keeps_recent_window_plus_milestones() {
        let root =
            std::env::temp_dir().join(format!("omiai-ret-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // steps 0, 50, 100, 101..110 (mỗi 100 là mốc)
        let all: Vec<u64> =
            std::iter::once(0).chain([50, 100]).chain(101..=110).collect();
        make_fake_checkpoints(&root, &all);

        let policy = RetentionPolicy { keep_recent: 5, milestone_every: 100 };
        let removed = apply_retention(&root, &policy).unwrap();

        // 13 step, giữ 5 gần nhất (106..110) + mốc {0, 100} → xoá 6: 50, 101..105
        let removed_steps: Vec<u64> = removed.iter().map(|(s, _)| *s).collect();
        assert_eq!(removed_steps, vec![50, 101, 102, 103, 104, 105]);

        let remaining: Vec<u64> =
            list_steps(&root).unwrap().into_iter().map(|(s, _)| s).collect();
        assert_eq!(remaining, vec![0, 100, 106, 107, 108, 109, 110]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn milestone_never_deleted_even_outside_window() {
        let root =
            std::env::temp_dir().join(format!("omiai-ret-m{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        make_fake_checkpoints(&root, &[0, 200, 201, 202]);

        let policy = RetentionPolicy { keep_recent: 1, milestone_every: 100 };
        apply_retention(&root, &policy).unwrap();

        let remaining: Vec<u64> =
            list_steps(&root).unwrap().into_iter().map(|(s, _)| s).collect();
        // 202 gần nhất giữ; 0 và 200 là mốc giữ; 201 bị xoá.
        assert_eq!(remaining, vec![0, 200, 202]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fewer_than_keep_recent_deletes_nothing() {
        let root =
            std::env::temp_dir().join(format!("omiai-ret-f{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        make_fake_checkpoints(&root, &[1, 2, 3]);
        let removed = apply_retention(&root, &RetentionPolicy::default()).unwrap();
        assert!(removed.is_empty());
        assert_eq!(list_steps(&root).unwrap().len(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }
}
