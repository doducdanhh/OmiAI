//! Active Inference / Free Energy Principle self-improvement loop and
//! genetic-programming-based self-modification proposals.
//!
//! The agent maintains a generative model of observations, computes
//! variational free energy (surprisal + complexity), and proposes
//! policy / code changes that reduce expected free energy.

use omiai_evolution::fitness::Fitness;
use omiai_evolution::genetic_programming::GeneticProgram;

/// Generative model parameters (mean / precision of Gaussian priors).
#[derive(Debug, Clone)]
pub struct GenerativeModel {
    pub prior_mean: Vec<f64>,
    pub prior_precision: Vec<f64>,
    pub likelihood_precision: f64,
}

impl GenerativeModel {
    pub fn new(dim: usize) -> Self {
        Self {
            prior_mean: vec![0.0; dim],
            prior_precision: vec![1.0; dim],
            likelihood_precision: 1.0,
        }
    }
}

/// Active Inference / metacognitive engine.
#[derive(Debug, Clone)]
pub struct MetaCognitiveEngine {
    pub model: GenerativeModel,
    pub free_energy_history: Vec<f64>,
}

impl MetaCognitiveEngine {
    pub fn new(dim: usize) -> Self {
        Self {
            model: GenerativeModel::new(dim),
            free_energy_history: Vec::new(),
        }
    }

    /// Variational free energy under a Laplace approximation:
    /// `F ≈ ½ Σ π_i (μ_i - m_i)² - ½ Σ ln π_i + ½ π_l Σ (o_i - μ_i)²`
    ///
    /// Complexity (KL from prior) + Accuracy (expected log-likelihood).
    pub fn free_energy(&self, beliefs: &[f64], observations: &[f64]) -> f64 {
        let n = beliefs
            .len()
            .min(self.model.prior_mean.len())
            .min(observations.len());
        let mut complexity = 0.0;
        let mut accuracy = 0.0;
        for i in 0..n {
            let pi = self.model.prior_precision[i].max(1e-9);
            let diff = beliefs[i] - self.model.prior_mean[i];
            complexity += 0.5 * pi * diff * diff - 0.5 * pi.ln();
            let err = observations[i] - beliefs[i];
            accuracy += 0.5 * self.model.likelihood_precision * err * err;
        }
        complexity + accuracy
    }

    /// Minimize free energy by gradient steps on beliefs (perception).
    pub fn minimize_surprisal(&mut self, observations: &[f64], steps: usize, lr: f64) -> Vec<f64> {
        let n = observations.len().min(self.model.prior_mean.len());
        let mut beliefs = self.model.prior_mean[..n].to_vec();
        for _ in 0..steps {
            for i in 0..n {
                let pi = self.model.prior_precision[i];
                let grad_complexity = pi * (beliefs[i] - self.model.prior_mean[i]);
                let grad_accuracy =
                    -self.model.likelihood_precision * (observations[i] - beliefs[i]);
                beliefs[i] -= lr * (grad_complexity + grad_accuracy);
            }
            let f = self.free_energy(&beliefs, observations);
            self.free_energy_history.push(f);
        }
        // Update prior means toward beliefs (learning)
        for i in 0..n {
            self.model.prior_mean[i] = 0.9 * self.model.prior_mean[i] + 0.1 * beliefs[i];
        }
        beliefs
    }

    /// Propose a self-modification: evolve a small CGP program that maps
    /// observation features → action scores; accept if free energy drops.
    pub fn rewrite_own_code<F>(
        &mut self,
        observation: &[f64],
        fitness_proxy: F,
        seed: u64,
    ) -> Option<GeneticProgram>
    where
        F: Fn(&GeneticProgram) -> Fitness + Sync,
    {
        let candidate = GeneticProgram::evolve(
            16,
            2,
            8,
            observation.len().max(1),
            6,
            1,
            &fitness_proxy,
            seed,
        );
        // Accept if fitness_proxy is high enough (proxy for lower free energy)
        if fitness_proxy(&candidate) > 0.5 {
            Some(candidate)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_energy_decreases() {
        let mut eng = MetaCognitiveEngine::new(2);
        let obs = vec![1.0, -1.0];
        eng.minimize_surprisal(&obs, 20, 0.1);
        assert!(eng.free_energy_history.len() >= 20);
        let first = eng.free_energy_history[0];
        let last = *eng.free_energy_history.last().unwrap();
        assert!(last <= first + 1e-6, "first={first} last={last}");
    }
}
