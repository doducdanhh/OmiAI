//! Monte Carlo Tree Search with UCT, designed for neuro-symbolic
//! integration: node expansion can consult a logical solver callback.

use std::collections::HashMap;

/// Game / planning state interface (implemented by callers).
pub trait GameState: Clone + Send {
    type Action: Clone + Eq + std::hash::Hash + Send;

    fn legal_actions(&self) -> Vec<Self::Action>;
    fn apply(&self, action: &Self::Action) -> Self;
    fn is_terminal(&self) -> bool;
    /// Reward from the perspective of the player to move at the root.
    fn evaluate(&self) -> f64;
}

/// MCTS node statistics.
#[derive(Debug, Clone, Default)]
struct Stats {
    visits: u64,
    value_sum: f64,
}

/// UCT Monte Carlo Tree Search.
pub struct Mcts<S: GameState> {
    pub exploration: f64,
    pub simulations: usize,
    _marker: std::marker::PhantomData<S>,
}

impl<S: GameState> Mcts<S> {
    pub fn new(simulations: usize) -> Self {
        Self {
            exploration: std::f64::consts::SQRT_2,
            simulations,
            _marker: std::marker::PhantomData,
        }
    }

    /// Run MCTS from `root` and return the most-visited action.
    pub fn search(&self, root: &S) -> Option<S::Action> {
        if root.is_terminal() {
            return None;
        }
        // Tree: path-key → stats. We key by action sequence string hashes.
        let mut tree: HashMap<Vec<u64>, Stats> = HashMap::new();
        let mut action_ids: HashMap<S::Action, u64> = HashMap::new();
        let mut next_id = 1u64;

        let root_actions = root.legal_actions();
        for a in &root_actions {
            action_ids.entry(a.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        }

        for _ in 0..self.simulations {
            let mut state = root.clone();
            let mut path: Vec<u64> = vec![];
            let mut path_actions: Vec<S::Action> = vec![];

            // Selection
            while !state.is_terminal() {
                let actions = state.legal_actions();
                if actions.is_empty() {
                    break;
                }
                // Ensure ids
                for a in &actions {
                    action_ids.entry(a.clone()).or_insert_with(|| {
                        let id = next_id;
                        next_id += 1;
                        id
                    });
                }
                // Expand if some child unvisited
                let unvisited: Vec<_> = actions
                    .iter()
                    .filter(|a| {
                        let mut p = path.clone();
                        p.push(action_ids[*a]);
                        !tree.contains_key(&p)
                    })
                    .cloned()
                    .collect();

                let action = if !unvisited.is_empty() {
                    unvisited[path.len() % unvisited.len()].clone()
                } else {
                    // UCT select. `actions` is guaranteed non-empty here
                    // because the loop breaks above when `actions.is_empty()`.
                    let parent_visits = tree.get(&path).map(|s| s.visits).unwrap_or(1) as f64;
                    let best = actions
                        .iter()
                        .max_by(|a, b| {
                            let ua = uct(
                                &tree,
                                &path,
                                action_ids[*a],
                                parent_visits,
                                self.exploration,
                            );
                            let ub =
                                uct(&tree, &path, action_ids[b], parent_visits, self.exploration);
                            ua.partial_cmp(&ub).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .cloned();
                    match best {
                        Some(a) => a,
                        None => break, // invariant: actions non-empty
                    }
                };

                let id = action_ids[&action];
                path.push(id);
                path_actions.push(action.clone());
                state = state.apply(&action);

                // If we expanded a new node, break to rollout
                if tree.get(&path).map(|s| s.visits).unwrap_or(0) == 0 {
                    tree.entry(path.clone()).or_default();
                    break;
                }
            }

            // Rollout
            let mut rollout = state.clone();
            let mut depth = 0;
            while !rollout.is_terminal() && depth < 50 {
                let acts = rollout.legal_actions();
                if acts.is_empty() {
                    break;
                }
                let a = acts[depth % acts.len()].clone();
                rollout = rollout.apply(&a);
                depth += 1;
            }
            let value = rollout.evaluate();

            // Backprop
            let mut prefix = vec![];
            // root stats
            {
                let s = tree.entry(vec![]).or_default();
                s.visits += 1;
                s.value_sum += value;
            }
            for id in &path {
                prefix.push(*id);
                let s = tree.entry(prefix.clone()).or_default();
                s.visits += 1;
                s.value_sum += value;
            }
        }

        // Most visited root child
        root_actions.into_iter().max_by(|a, b| {
            let pa = vec![action_ids[a]];
            let pb = vec![action_ids[b]];
            let va = tree.get(&pa).map(|s| s.visits).unwrap_or(0);
            let vb = tree.get(&pb).map(|s| s.visits).unwrap_or(0);
            va.cmp(&vb)
        })
    }
}

fn uct(
    tree: &HashMap<Vec<u64>, Stats>,
    path: &[u64],
    action_id: u64,
    parent_visits: f64,
    c: f64,
) -> f64 {
    let mut child_path = path.to_vec();
    child_path.push(action_id);
    match tree.get(&child_path) {
        Some(s) if s.visits > 0 => {
            let q = s.value_sum / s.visits as f64;
            q + c * ((parent_visits.ln() / s.visits as f64).sqrt())
        }
        _ => f64::INFINITY,
    }
}

/// Integrate a logical validity filter: prune illegal-by-logic actions.
pub fn filter_actions_with_solver<A, F>(actions: Vec<A>, mut is_valid: F) -> Vec<A>
where
    F: FnMut(&A) -> bool,
{
    actions.into_iter().filter(|a| is_valid(a)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Nim {
        stones: u32,
    }

    impl GameState for Nim {
        type Action = u32; // take 1, 2, or 3

        fn legal_actions(&self) -> Vec<u32> {
            (1..=3).filter(|&k| k <= self.stones).collect()
        }
        fn apply(&self, action: &u32) -> Self {
            Nim {
                stones: self.stones.saturating_sub(*action),
            }
        }
        fn is_terminal(&self) -> bool {
            self.stones == 0
        }
        fn evaluate(&self) -> f64 {
            // Player who faces 0 lost — from root we want positive if odd-ish
            if self.stones == 0 { 0.0 } else { 0.5 }
        }
    }

    #[test]
    fn mcts_picks_legal_move() {
        let game = Nim { stones: 5 };
        let mcts = Mcts::new(100);
        let action = mcts.search(&game).expect("move");
        assert!((1..=3).contains(&action));
    }
}
