//! Action interface: discrete action sets and execution logs for the
//! agent loop (perception → reason → act).

use serde::{Deserialize, Serialize};

/// A discrete action the agent can execute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    pub args: Vec<String>,
}

impl Action {
    pub fn new(name: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }

    pub fn unit(name: impl Into<String>) -> Self {
        Self::new(name, vec![])
    }
}

/// Log of executed actions with optional outcomes.
#[derive(Debug, Default, Clone)]
pub struct ActionLog {
    pub entries: Vec<(Action, Option<String>)>,
}

impl ActionLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, action: Action, outcome: Option<String>) {
        self.entries.push((action, outcome));
    }

    pub fn last(&self) -> Option<&(Action, Option<String>)> {
        self.entries.last()
    }
}

/// Select action by index into a menu (safe).
pub fn select_action(menu: &[Action], index: usize) -> Option<&Action> {
    menu.get(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_records() {
        let mut log = ActionLog::new();
        log.record(Action::unit("noop"), Some("ok".into()));
        assert_eq!(log.entries.len(), 1);
    }
}
