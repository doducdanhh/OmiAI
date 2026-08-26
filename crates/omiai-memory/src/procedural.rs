//! Procedural memory: skills as named action sequences with preconditions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A skill: precondition atoms + ordered action steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub preconditions: Vec<String>,
    pub steps: Vec<String>,
    /// Success count for reinforcement-free reliability estimate
    pub successes: u64,
    pub attempts: u64,
}

impl Skill {
    pub fn reliability(&self) -> f64 {
        if self.attempts == 0 {
            0.5
        } else {
            self.successes as f64 / self.attempts as f64
        }
    }

    pub fn applicable(&self, world_facts: &[String]) -> bool {
        let set: std::collections::HashSet<&str> = world_facts.iter().map(|s| s.as_str()).collect();
        self.preconditions.iter().all(|p| set.contains(p.as_str()))
    }
}

/// Procedural memory store.
#[derive(Debug, Default, Clone)]
pub struct ProceduralMemory {
    pub skills: HashMap<String, Skill>,
}

impl ProceduralMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn learn(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Select the most reliable applicable skill for the current world.
    pub fn select(&self, world_facts: &[String]) -> Option<&Skill> {
        self.skills
            .values()
            .filter(|s| s.applicable(world_facts))
            .max_by(|a, b| {
                a.reliability()
                    .partial_cmp(&b.reliability())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    pub fn record_outcome(&mut self, name: &str, success: bool) {
        if let Some(s) = self.skills.get_mut(name) {
            s.attempts += 1;
            if success {
                s.successes += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_applicable() {
        let mut mem = ProceduralMemory::new();
        mem.learn(Skill {
            name: "open_door".into(),
            preconditions: vec!["at_door".into(), "has_key".into()],
            steps: vec!["unlock".into(), "push".into()],
            successes: 5,
            attempts: 5,
        });
        assert!(mem.select(&["at_door".into()]).is_none());
        assert!(mem.select(&["at_door".into(), "has_key".into()]).is_some());
    }
}
