//! Partially Observable Markov Decision Processes (POMDPs): belief-state
//! updates and a simple value-iteration sketch over discrete beliefs.

use std::collections::HashMap;

/// Discrete POMDP with finite states, actions, and observations.
#[derive(Debug, Clone)]
pub struct Pomdp {
    pub n_states: usize,
    pub n_actions: usize,
    pub n_obs: usize,
    /// T[a][s][s'] = P(s'|s,a)
    pub transition: Vec<Vec<Vec<f64>>>,
    /// O[a][s'][o] = P(o|s',a)
    pub observation: Vec<Vec<Vec<f64>>>,
    /// R[a][s] reward
    pub reward: Vec<Vec<f64>>,
    pub gamma: f64,
}

impl Pomdp {
    /// Uniform initial belief.
    pub fn uniform_belief(&self) -> Vec<f64> {
        vec![1.0 / self.n_states as f64; self.n_states]
    }

    /// Bayesian belief update:  
    /// `b'(s') ∝ O(o|s',a) Σ_s T(s'|s,a) b(s)`
    pub fn update_belief(&self, belief: &[f64], action: usize, obs: usize) -> Vec<f64> {
        let a = action.min(self.n_actions.saturating_sub(1));
        let mut next = vec![0.0; self.n_states];
        for sp in 0..self.n_states {
            let mut sum = 0.0;
            for s in 0..self.n_states {
                let t = self.transition[a][s][sp];
                sum += t * belief.get(s).copied().unwrap_or(0.0);
            }
            let o = self.observation[a][sp].get(obs).copied().unwrap_or(0.0);
            next[sp] = o * sum;
        }
        let z: f64 = next.iter().sum();
        if z < 1e-15 {
            return self.uniform_belief();
        }
        for x in next.iter_mut() {
            *x /= z;
        }
        next
    }

    /// Expected reward under belief for action a.
    pub fn expected_reward(&self, belief: &[f64], action: usize) -> f64 {
        let a = action.min(self.n_actions.saturating_sub(1));
        belief
            .iter()
            .enumerate()
            .map(|(s, b)| b * self.reward[a].get(s).copied().unwrap_or(0.0))
            .sum()
    }

    /// Greedy one-step action under current belief.
    pub fn greedy_action(&self, belief: &[f64]) -> usize {
        (0..self.n_actions)
            .max_by(|&a, &b| {
                self.expected_reward(belief, a)
                    .partial_cmp(&self.expected_reward(belief, b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0)
    }
}

/// Build a tiny 2-state tiger-like POMDP for tests / demos.
pub fn tiger_pomdp() -> Pomdp {
    // states: 0=tiger-left, 1=tiger-right
    // actions: 0=listen, 1=open-left, 2=open-right
    // obs: 0=hear-left, 1=hear-right
    let n_s = 2;
    let n_a = 3;
    let n_o = 2;
    let mut transition = vec![vec![vec![0.0; n_s]; n_s]; n_a];
    // listen: stay
    for s in 0..n_s {
        transition[0][s][s] = 1.0;
    }
    // open: reset to uniform-ish (absorb random)
    for a in 1..n_a {
        for s in 0..n_s {
            transition[a][s][0] = 0.5;
            transition[a][s][1] = 0.5;
        }
    }
    let mut observation = vec![vec![vec![0.0; n_o]; n_s]; n_a];
    // listen: 85% correct
    observation[0][0][0] = 0.85;
    observation[0][0][1] = 0.15;
    observation[0][1][0] = 0.15;
    observation[0][1][1] = 0.85;
    for a in 1..n_a {
        for s in 0..n_s {
            observation[a][s][0] = 0.5;
            observation[a][s][1] = 0.5;
        }
    }
    let mut reward = vec![vec![0.0; n_s]; n_a];
    reward[0][0] = -1.0;
    reward[0][1] = -1.0;
    reward[1][0] = -100.0; // open left, tiger left
    reward[1][1] = 10.0;
    reward[2][0] = 10.0;
    reward[2][1] = -100.0;

    Pomdp {
        n_states: n_s,
        n_actions: n_a,
        n_obs: n_o,
        transition,
        observation,
        reward,
        gamma: 0.95,
    }
}

/// MDP value iteration (fully observable special case).
pub fn value_iteration(
    n_states: usize,
    n_actions: usize,
    transition: &[Vec<Vec<f64>>],
    reward: &[Vec<f64>],
    gamma: f64,
    iters: usize,
) -> (Vec<f64>, Vec<usize>) {
    let mut v = vec![0.0; n_states];
    let mut policy = vec![0usize; n_states];
    for _ in 0..iters {
        let mut nv = vec![0.0; n_states];
        for s in 0..n_states {
            let mut best = f64::NEG_INFINITY;
            let mut best_a = 0;
            for a in 0..n_actions {
                let mut q = reward[a][s];
                for sp in 0..n_states {
                    q += gamma * transition[a][s][sp] * v[sp];
                }
                if q > best {
                    best = q;
                    best_a = a;
                }
            }
            nv[s] = best;
            policy[s] = best_a;
        }
        v = nv;
    }
    (v, policy)
}

/// Belief hash key for tabular POMDP approximations.
pub fn discretize_belief(belief: &[f64], bins: usize) -> Vec<u16> {
    belief
        .iter()
        .map(|p| ((p.clamp(0.0, 1.0) * bins as f64).floor() as u16).min(bins as u16))
        .collect()
}

pub type BeliefTable = HashMap<Vec<u16>, f64>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belief_update_normalizes() {
        let pomdp = tiger_pomdp();
        let b = pomdp.uniform_belief();
        let b2 = pomdp.update_belief(&b, 0, 0);
        let sum: f64 = b2.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!(b2[0] > b2[1]); // heard left ⇒ tiger more likely left
    }
}
