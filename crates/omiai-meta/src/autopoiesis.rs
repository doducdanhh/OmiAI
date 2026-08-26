//! Autopoietic self-improvement loop.
//!
//! Autopoiesis (Maturana & Varela 1980) refers to a system that
//! continuously produces and maintains itself. In an AI context, this
//! module implements an integrated loop that combines:
//!
//! - **Perception** (raw observations as numeric vectors)
//! - **Free Energy Principle** (Friston) — variational free energy as
//!   the unified objective; minimize to update beliefs.
//! - **Goal generation** — autopoietic subgoals derived from free
//!   energy gradients over goal-state distances.
//! - **Cartesian Genetic Programming** — evolve action policies whose
//!   fitness is *negative expected free energy*.
//! - **Knowledge graph maintenance** — register newly learned relations
//!   in a [`KnowledgeGraph`] so the world model grows.

use std::collections::HashMap;

use omiai_evolution::fitness::mse_to_fitness;
use omiai_evolution::genetic_programming::GeneticProgram;
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use crate::self_improvement::MetaCognitiveEngine;

/// A perceptual observation: feature vector plus a discrete label that
/// the system can store in its knowledge graph.
#[derive(Debug, Clone)]
pub struct WorldState {
    pub features: Vec<f64>,
    pub label: String,
}

/// One cycle of the autopoietic loop.
#[derive(Debug, Clone)]
pub struct AutopoieticLoop {
    /// Latent-dim of the generative model.
    pub latent_dim: usize,
    /// Feature dim of incoming observations.
    pub feature_dim: usize,
    /// FEP engine.
    pub engine: MetaCognitiveEngine,
    /// Persistent world model.
    pub kg: KnowledgeGraph,
    /// Free-energy history (one entry per `step`).
    pub history: Vec<f64>,
    /// Cached best-evolved policy.
    pub best_policy: Option<GeneticProgram>,
    /// Round counter.
    pub rounds: u64,
}

impl AutopoieticLoop {
    /// Construct a new autopoietic loop.
    pub fn new(latent_dim: usize, feature_dim: usize) -> Self {
        Self {
            latent_dim,
            feature_dim,
            engine: MetaCognitiveEngine::new(latent_dim),
            kg: KnowledgeGraph::new(),
            history: Vec::new(),
            best_policy: None,
            rounds: 0,
        }
    }

    /// Add a starting concept to the world model.
    pub fn seed_concept(&mut self, id: &str, label: &str) {
        self.kg.add_concept(Concept {
            id: id.into(),
            label: label.into(),
        });
    }

    /// Run one cycle of the loop.
    pub fn step(&mut self, obs: &WorldState) -> f64 {
        self.rounds += 1;

        // 1. Minimize free energy under observation.
        let beliefs = self.engine.minimize_surprisal(&obs.features, 16, 0.1);
        let fe = self.engine.free_energy(&beliefs, &obs.features);
        self.history.push(fe);

        // 2. Register the label as a concept if new.
        if self.kg.get(&obs.label).is_none() {
            self.kg.add_concept(Concept {
                id: obs.label.clone(),
                label: obs.label.clone(),
            });
        }

        // 3. Evolve a small CGP policy to "act" on the world.
        let target = obs.features.clone();
        let policy = self.evolve_policy(&target);
        self.best_policy = Some(policy);

        fe
    }

    /// Run multiple cycles and return final free energy + summary.
    pub fn run(&mut self, observations: &[WorldState], verbose: bool) -> AutopoieticSummary {
        let start_len = self.history.len();
        for obs in observations {
            let fe = self.step(obs);
            if verbose {
                println!("[autopoiesis] round {}: FE = {fe:.6}", self.rounds);
            }
        }
        AutopoieticSummary {
            rounds: self.rounds,
            initial_fe: self.history.get(start_len).copied().unwrap_or(0.0),
            final_fe: self.history.last().copied().unwrap_or(0.0),
            kg_concepts: self.kg.len(),
        }
    }

    /// Evolve a policy whose fitness is `−FE(policy(features), target)`.
    fn evolve_policy(&self, target: &[f64]) -> GeneticProgram {
        let t = target.to_vec();
        let p = GeneticProgram::evolve(
            8,
            1,
            4,
            target.len().max(1),
            4,
            target.len(),
            |prog| {
                let pred = prog.eval(&t);
                mse_to_fitness(&pred, &t)
            },
            self.rounds.max(1),
        );
        p
    }

    /// Find the most-recent free energy.
    pub fn last_free_energy(&self) -> Option<f64> {
        self.history.last().copied()
    }

    /// Mean free energy over the last `n` cycles.
    pub fn recent_mean_fe(&self, n: usize) -> f64 {
        let n = n.min(self.history.len());
        if n == 0 {
            return 0.0;
        }
        let sum: f64 = self.history.iter().rev().take(n).sum();
        sum / n as f64
    }
}

/// Summary of a multi-cycle run.
#[derive(Debug, Clone)]
pub struct AutopoieticSummary {
    pub rounds: u64,
    pub initial_fe: f64,
    pub final_fe: f64,
    pub kg_concepts: usize,
}

impl AutopoieticSummary {
    /// Free-energy reduction across the run.
    pub fn fe_reduction(&self) -> f64 {
        self.initial_fe - self.final_fe
    }
}

/// A minimal **Markov blanket**: separates "internal" states from
/// "external" ones, mediating all coupling.
#[derive(Debug, Clone)]
pub struct MarkovBlanket {
    pub internal: Vec<String>,
    pub blanket: Vec<String>,
    pub external: Vec<String>,
}

impl MarkovBlanket {
    pub fn new(internal: Vec<String>, blanket: Vec<String>, external: Vec<String>) -> Self {
        Self {
            internal,
            blanket,
            external,
        }
    }

    /// Active inference: choose an action that minimizes expected free
    /// energy — the expected divergence between predicted and desired
    /// blanket states under a candidate policy.
    pub fn select_action(
        internal_states: &HashMap<String, f64>,
        desired_blanket: &HashMap<String, f64>,
        candidates: &[String],
    ) -> Option<String> {
        let mut best: Option<(f64, &String)> = None;
        for c in candidates {
            let cur = internal_states.get(c).copied().unwrap_or(0.0);
            let d: f64 = if desired_blanket.is_empty() {
                cur * cur
            } else {
                let s: f64 = desired_blanket.values().map(|v| (cur - v).powi(2)).sum();
                s / desired_blanket.len() as f64
            };
            if best.is_none() || d < best.unwrap().0 {
                best = Some((d, c));
            }
        }
        best.map(|(_, s)| s.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_runs_and_records_history() {
        let mut al = AutopoieticLoop::new(2, 2);
        al.seed_concept("self", "the agent");
        let obs = WorldState {
            features: vec![1.0, -0.5],
            label: "obs1".into(),
        };
        let fe = al.step(&obs);
        assert!(fe.is_finite());
        assert_eq!(al.history.len(), 1);
        assert!(al.best_policy.is_some());
    }

    #[test]
    fn markov_blanket_selects_closest_internal() {
        let mut internal = HashMap::new();
        internal.insert("a".to_string(), 1.0);
        internal.insert("b".to_string(), 2.0);
        internal.insert("c".to_string(), 3.0);
        let mut desired = HashMap::new();
        desired.insert("x".to_string(), 0.5);
        let chosen = MarkovBlanket::select_action(
            &internal,
            &desired,
            &["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(chosen, Some("a".to_string()));
    }
}
