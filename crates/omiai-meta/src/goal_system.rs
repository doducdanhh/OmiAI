//! Hierarchical goal management with autopoietic subgoal generation.
//!
//! Goals form a tree; unsatisfied parents spawn subgoals. This is the
//! drive system that replaces external reward functions (active inference
//! / autopoiesis).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A hierarchical goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub parent: Option<String>,
    pub priority: f64,
    pub satisfied: bool,
    pub children: Vec<String>,
}

/// Goal system (forest of goal trees).
#[derive(Debug, Default, Clone)]
pub struct GoalSystem {
    pub goals: Vec<Goal>,
}

impl GoalSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a root or child goal.
    pub fn add_goal(
        &mut self,
        description: impl Into<String>,
        parent: Option<String>,
        priority: f64,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        if let Some(ref p) = parent {
            if let Some(pg) = self.goals.iter_mut().find(|g| g.id == *p) {
                pg.children.push(id.clone());
            }
        }
        self.goals.push(Goal {
            id: id.clone(),
            description: description.into(),
            parent,
            priority,
            satisfied: false,
            children: vec![],
        });
        id
    }

    pub fn mark_satisfied(&mut self, id: &str) {
        if let Some(g) = self.goals.iter_mut().find(|g| g.id == id) {
            g.satisfied = true;
        }
        // Propagate: parent satisfied if all children are
        self.propagate_satisfaction();
    }

    fn propagate_satisfaction(&mut self) {
        // Multiple passes for deep trees
        for _ in 0..self.goals.len() {
            let snapshot: Vec<(String, bool)> = self
                .goals
                .iter()
                .map(|g| {
                    if g.children.is_empty() {
                        (g.id.clone(), g.satisfied)
                    } else {
                        let all = g.children.iter().all(|c| {
                            self.goals
                                .iter()
                                .find(|x| x.id == *c)
                                .map(|x| x.satisfied)
                                .unwrap_or(false)
                        });
                        (g.id.clone(), all)
                    }
                })
                .collect();
            for (id, sat) in snapshot {
                if let Some(g) = self.goals.iter_mut().find(|g| g.id == id) {
                    if !g.children.is_empty() {
                        g.satisfied = sat;
                    }
                }
            }
        }
    }

    /// Autopoietic subgoal generation: for each unsatisfied root/branch,
    /// spawn a generic "achieve: …" subgoal if none exist yet.
    pub fn generate_subgoals(&mut self) -> Vec<String> {
        let mut created = Vec::new();
        let unsatisfied: Vec<(String, String, f64)> = self
            .goals
            .iter()
            .filter(|g| !g.satisfied)
            .map(|g| (g.id.clone(), g.description.clone(), g.priority))
            .collect();

        for (id, desc, prio) in unsatisfied {
            let has_open_child = self
                .goals
                .iter()
                .any(|g| g.parent.as_deref() == Some(id.as_str()) && !g.satisfied);
            if !has_open_child {
                let child = self.add_goal(format!("achieve: {desc}"), Some(id), prio * 0.9);
                created.push(child);
            }
        }
        created
    }

    /// Highest-priority unsatisfied leaf goal (ready for action).
    pub fn next_goal(&self) -> Option<&Goal> {
        self.goals
            .iter()
            .filter(|g| !g.satisfied && g.children.is_empty())
            .max_by(|a, b| {
                a.priority
                    .partial_cmp(&b.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_and_subgoals() {
        let mut gs = GoalSystem::new();
        let root = gs.add_goal("survive", None, 1.0);
        let kids = gs.generate_subgoals();
        assert!(!kids.is_empty());
        assert!(gs.next_goal().is_some());
        let leaf = gs.next_goal().unwrap().id.clone();
        gs.mark_satisfied(&leaf);
        // After leaf done, may still have work
        let _ = root;
    }
}
