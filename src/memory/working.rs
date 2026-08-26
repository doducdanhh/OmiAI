//! Working memory based on Global Workspace Theory (Baars): a capacity-
//! limited blackboard that broadcasts the most salient content.

use std::collections::VecDeque;

/// An item in the global workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceItem {
    pub content: String,
    pub salience: f64,
}

/// Capacity-limited working memory / global workspace.
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    pub capacity: usize,
    items: VecDeque<WorkspaceItem>,
}

impl WorkingMemory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            items: VecDeque::new(),
        }
    }

    /// Broadcast: insert item; if over capacity, evict lowest salience.
    pub fn broadcast(&mut self, item: WorkspaceItem) {
        self.items.push_back(item);
        while self.items.len() > self.capacity {
            // Evict minimum salience
            let mut min_i = 0;
            let mut min_s = f64::INFINITY;
            for (i, it) in self.items.iter().enumerate() {
                if it.salience < min_s {
                    min_s = it.salience;
                    min_i = i;
                }
            }
            self.items.remove(min_i);
        }
    }

    /// Currently conscious (workspace) contents, highest salience first.
    pub fn contents(&self) -> Vec<&WorkspaceItem> {
        let mut v: Vec<_> = self.items.iter().collect();
        v.sort_by(|a, b| {
            b.salience
                .partial_cmp(&a.salience)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::new(7) // Miller's magical number ±2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_evicts_low_salience() {
        let mut wm = WorkingMemory::new(2);
        wm.broadcast(WorkspaceItem {
            content: "a".into(),
            salience: 0.1,
        });
        wm.broadcast(WorkspaceItem {
            content: "b".into(),
            salience: 0.5,
        });
        wm.broadcast(WorkspaceItem {
            content: "c".into(),
            salience: 0.9,
        });
        assert_eq!(wm.len(), 2);
        let contents: Vec<_> = wm.contents().iter().map(|i| i.content.as_str()).collect();
        assert!(contents.contains(&"c"));
        assert!(!contents.contains(&"a"));
    }
}
