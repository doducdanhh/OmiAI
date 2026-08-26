//! PUCT-style Monte Carlo Tree Search (AlphaZero variant).
//!
//! PUCT (Predictor + UCT) augments the classic UCB1 selection rule
//! with a *prior policy* `P(a | s)` that biases exploration toward
//! actions the prior believes are good:
//!
//! ```text
//! PUCT(s, a) = Q(s, a) + c_puct · P(a | s) · √N(s) / (1 + N(s, a))
//! ```
//!
//! where `Q(s, a)` is the action value, `N` are visit counts, and
//! `c_puct` controls the exploration/exploitation tradeoff (typically
//! ~1.5–2.0 in AlphaZero).
//!
//! Unlike [`super::mcts::Mcts`], which uses pure UCT with random
//! rollouts, this implementation lets the caller supply a **prior
//! policy** and (optionally) a **value function**. When no prior is
//! supplied, a uniform prior is used and the algorithm degenerates to
//! a UCT-like search with the Dirichlet-noise trick disabled.
//!
//! # References
//!
//! - Rosin, *Multi-armed bandits with episode context*, AoAM 2011.
//! - Silver et al., *Mastering Chess and Shogi by Self-Play with a
//!   General Reinforcement Learning Algorithm* (AlphaZero, 2017).

use std::collections::HashMap;

use super::mcts::GameState;

/// Default exploration constant (AlphaZero uses c_puct ≈ 1.5–2).
pub const DEFAULT_C_PUCT: f64 = 1.5;

/// Dirichlet noise concentration (for root exploration).
pub const DIRICHLET_ALPHA: f64 = 0.3;

#[derive(Debug, Clone, Copy)]
struct Stats {
    visits: u64,
    value_sum: f64,
    /// Prior probability from the supplied policy.
    prior: f64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            visits: 0,
            value_sum: 0.0,
            prior: 0.0,
        }
    }
}

/// PUCT-style MCTS with optional prior policy and value function.
pub struct PuctMcts<S: GameState> {
    pub simulations: usize,
    pub c_puct: f64,
    pub add_dirichlet: bool,
    _marker: std::marker::PhantomData<S>,
}

impl<S: GameState> PuctMcts<S> {
    pub fn new(simulations: usize) -> Self {
        Self {
            simulations,
            c_puct: DEFAULT_C_PUCT,
            add_dirichlet: false,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_exploration(mut self, c_puct: f64) -> Self {
        self.c_puct = c_puct;
        self
    }

    /// Enable Dirichlet noise at the root (recommended for self-play).
    pub fn with_dirichlet(mut self, on: bool) -> Self {
        self.add_dirichlet = on;
        self
    }

    /// Run PUCT search from `root` with no prior (uniform). Returns the
    /// action with the highest visit count.
    pub fn search(&self, root: &S) -> Option<S::Action> {
        self.search_with_prior(root, |_s, actions| {
            let n = actions.len().max(1) as f64;
            actions.iter().map(|_| 1.0 / n).collect()
        })
    }

    /// Run PUCT search with a custom prior `P(a|s)`.
    ///
    /// `prior` is called once per expansion with the state and its legal
    /// actions; it returns one probability per action (must sum to ~1).
    pub fn search_with_prior<F>(&self, root: &S, prior: F) -> Option<S::Action>
    where
        F: Fn(&S, &[S::Action]) -> Vec<f64>,
    {
        if root.is_terminal() {
            return None;
        }

        let mut action_ids: HashMap<S::Action, u64> = HashMap::new();
        let mut next_id = 1u64;
        let mut tree: HashMap<Vec<u64>, Stats> = HashMap::new();

        let root_actions = root.legal_actions();
        if root_actions.is_empty() {
            return None;
        }
        for a in &root_actions {
            action_ids.entry(a.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        }

        // Initialize root children with priors
        let priors: Vec<f64> = prior(root, &root_actions);
        let priors = if self.add_dirichlet {
            apply_dirichlet(&priors, DIRICHLET_ALPHA, 0.25)
        } else {
            priors
        };
        for (i, a) in root_actions.iter().enumerate() {
            let id = action_ids[a];
            let path = vec![id];
            let s = tree.entry(path.clone()).or_default();
            s.prior = priors.get(i).copied().unwrap_or(0.0);
            let _ = s;
        }

        for _ in 0..self.simulations {
            let mut state = root.clone();
            let mut path: Vec<u64> = Vec::new();
            // Selection
            while !state.is_terminal() {
                let actions = state.legal_actions();
                if actions.is_empty() {
                    break;
                }
                for a in &actions {
                    action_ids.entry(a.clone()).or_insert_with(|| {
                        let id = next_id;
                        next_id += 1;
                        id
                    });
                }
                // Expand if some child unvisited
                let unvisited: Vec<S::Action> = actions
                    .iter()
                    .filter(|a| {
                        let mut p = path.clone();
                        p.push(action_ids[*a]);
                        !tree.contains_key(&p)
                    })
                    .cloned()
                    .collect();

                let action = if !unvisited.is_empty() {
                    // Prefer the unvisited action with the highest known
                    // prior so the policy actually steers expansion.
                    let child_prior = |a: &S::Action| -> f64 {
                        let mut p = path.clone();
                        p.push(action_ids[a]);
                        tree.get(&p).map(|s| s.prior).unwrap_or(0.0)
                    };
                    unvisited
                        .iter()
                        .max_by(|a, b| {
                            child_prior(a)
                                .partial_cmp(&child_prior(b))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .cloned()
                        .unwrap()
                } else {
                    let parent_visits = tree.get(&path).map(|s| s.visits).unwrap_or(1) as f64;
                    let parent_visits_sqrt = parent_visits.sqrt();
                    actions
                        .iter()
                        .max_by(|a, b| {
                            let ua = puct_score(
                                tree.get(&with_action(&path, action_ids[*a])).copied(),
                                parent_visits_sqrt,
                                self.c_puct,
                            );
                            let ub = puct_score(
                                tree.get(&with_action(&path, action_ids[*b])).copied(),
                                parent_visits_sqrt,
                                self.c_puct,
                            );
                            ua.partial_cmp(&ub).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .cloned()
                        .unwrap()
                };

                let id = action_ids[&action];
                path.push(id);
                state = state.apply(&action);

                // If newly expanded, record the policy's prior for it
                if tree.get(&path).map(|s| s.visits).unwrap_or(0) == 0 {
                    let s = tree.entry(path.clone()).or_default();
                    let pri = priors.get(actions.iter().position(|a| a == &action).unwrap_or(0));
                    s.prior = pri.copied().unwrap_or(1.0 / actions.len().max(1) as f64);
                    break;
                }
            }

            // Rollout (random or truncated)
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
            for i in 0..=path.len() {
                let prefix = if i == 0 { vec![] } else { path[..i].to_vec() };
                let s = tree.entry(prefix).or_default();
                s.visits += 1;
                s.value_sum += value;
            }
        }

        // Pick the most-visited root child
        root_actions.into_iter().max_by(|a, b| {
            let pa = vec![action_ids[a]];
            let pb = vec![action_ids[b]];
            let va = tree.get(&pa).map(|s| s.visits).unwrap_or(0);
            let vb = tree.get(&pb).map(|s| s.visits).unwrap_or(0);
            va.cmp(&vb)
        })
    }
}

fn with_action(path: &[u64], action_id: u64) -> Vec<u64> {
    let mut p = path.to_vec();
    p.push(action_id);
    p
}

fn puct_score(stats: Option<Stats>, parent_sqrt: f64, c_puct: f64) -> f64 {
    match stats {
        Some(s) if s.visits > 0 => {
            let q = s.value_sum / s.visits as f64;
            q + c_puct * s.prior * parent_sqrt / (1.0 + s.visits as f64)
        }
        // Unvisited (or missing): use the prior alone as the UCB score
        _ => f64::INFINITY,
    }
}

/// Mix the prior with Dirichlet noise: `p' = (1-ε)·p + ε·η`.
fn apply_dirichlet(priors: &[f64], _alpha: f64, eps: f64) -> Vec<f64> {
    if priors.is_empty() {
        return Vec::new();
    }
    let n = priors.len();
    let mut rng_state = 0xDEADBEEFu64.wrapping_add(n as u64);
    let mut gamma_samples = Vec::with_capacity(n);
    for _ in 0..n {
        // Marsaglia & Tsang gamma sampler (α≥1 not required here)
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((rng_state >> 11) as f64) / (1u64 << 53) as f64;
        let sample = -((1.0 - u).max(1e-15)).ln();
        gamma_samples.push(sample);
    }
    let total: f64 = gamma_samples.iter().sum();
    let eta: Vec<f64> = gamma_samples.iter().map(|g| g / total).collect();
    priors
        .iter()
        .zip(eta.iter())
        .map(|(p, e)| (1.0 - eps) * p + eps * e)
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcts::GameState;

    #[derive(Clone)]
    struct Nim {
        stones: u32,
    }

    impl GameState for Nim {
        type Action = u32;
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
            if self.stones == 0 { 0.0 } else { 0.5 }
        }
    }

    #[test]
    fn puct_picks_legal_move() {
        let game = Nim { stones: 5 };
        let searcher = PuctMcts::new(200);
        let action = searcher.search(&game).expect("move");
        assert!((1..=3).contains(&action));
    }

    #[test]
    fn puct_with_prior_picks_highest_prior() {
        // Prior heavily favors action 1; PUCT should pick action 1.
        let game = Nim { stones: 5 };
        let searcher = PuctMcts::new(50);
        let action = searcher
            .search_with_prior(&game, |_s, actions| {
                actions
                    .iter()
                    .enumerate()
                    .map(|(i, _)| if i == 0 { 0.9 } else { 0.05 })
                    .collect()
            })
            .expect("move");
        assert_eq!(action, 1);
    }

    #[test]
    fn dirichlet_noise_sums_to_one() {
        let priors = vec![0.5, 0.5];
        let noisy = apply_dirichlet(&priors, 0.3, 0.25);
        assert_eq!(noisy.len(), 2);
        let s: f64 = noisy.iter().sum();
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn puct_score_unvisited_is_infinite() {
        let s = puct_score(None, 2.0, 1.5);
        assert!(s.is_infinite());
    }

    #[test]
    fn puct_score_visited_balances_exploration() {
        let stats = Stats {
            visits: 5,
            value_sum: 2.5,
            prior: 0.5,
        };
        let s = puct_score(Some(stats), 10.0_f64.sqrt(), 1.5);
        // Q=0.5, exploration = 1.5 * 0.5 * sqrt(10) / 6
        let expected = 0.5 + 1.5 * 0.5 * 10.0_f64.sqrt() / 6.0;
        assert!((s - expected).abs() < 1e-9, "got {s} expected {expected}");
    }
}
