//! Gibbs sampling for discrete Bayesian networks.
//!
//! Gibbs sampling (Geman & Geman 1984) is a Markov Chain Monte Carlo
//! method that samples each variable in turn from its full conditional
//! `P(X_i | X_{-i})`, using the current values of all other variables
//! as evidence.
//!
//! For a discrete Bayesian network where each variable has a small
//! finite domain, the conditional probability `P(X_i = v | evidence)`
//! can be computed in closed form by enumerating the BN's joint
//! distribution restricted to the evidence.
//!
//! # Algorithm
//!
//! 1. Initialize each variable to an arbitrary value.
//! 2. For each iteration:
//!    a. Pick a variable `X_i` (random or sequential).
//!    b. Compute `P(X_i = v | current assignment of all other vars)`.
//!    c. Sample `X_i` from this distribution.
//! 3. Discard burn-in samples; the remainder approximate the posterior.
//!
//! # References
//!
//! - Geman & Geman, *Stochastic Relaxation, Gibbs Distributions, and the
//!   Bayesian Restoration of Images* (1984).
//! - Casella & George, *Explaining the Gibbs Sampler* (Am. Stat. 1992).

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

use super::bayesian::{BayesianNetwork, Cpt};

/// Configuration for the Gibbs sampler.
#[derive(Debug, Clone)]
pub struct GibbsConfig {
    /// Number of iterations (including burn-in).
    pub iterations: usize,
    /// Number of initial iterations discarded as burn-in.
    pub burn_in: usize,
    /// Lag between recorded samples (reduces autocorrelation).
    pub thinning: usize,
}

impl Default for GibbsConfig {
    fn default() -> Self {
        Self {
            iterations: 2000,
            burn_in: 500,
            thinning: 2,
        }
    }
}

/// Result of a Gibbs sampling run.
#[derive(Debug, Clone)]
pub struct GibbsResult {
    /// Post-burn-in, thinned samples: `samples[i]` is the i-th recorded
    /// full assignment (variable_name → value).
    pub samples: Vec<HashMap<String, bool>>,
    /// Empirical marginal probability for each variable (P(var=true)).
    pub marginals: HashMap<String, f64>,
    /// Iteration count actually run.
    pub iterations: usize,
}

/// Run Gibbs sampling on a discrete Bayesian network.
pub fn gibbs_sample(
    bn: &BayesianNetwork,
    evidence: &HashMap<String, bool>,
    config: &GibbsConfig,
    seed: u64,
) -> GibbsResult {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let var_names: Vec<String> = bn.nodes.iter().map(|n| n.variable.clone()).collect();
    let n = var_names.len();

    // Initialize assignment: evidence where present, else false
    let mut assignment: HashMap<String, bool> = HashMap::new();
    for v in &var_names {
        assignment.insert(v.clone(), evidence.get(v).copied().unwrap_or(false));
    }

    let mut all_samples: Vec<HashMap<String, bool>> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();

    let mut record_count = 0usize;
    for it in 0..config.iterations {
        // Update each variable in random order
        let mut order: Vec<usize> = (0..n).collect();
        // Fisher-Yates shuffle
        for i in (1..order.len()).rev() {
            let j = rng.r#gen_range(0..=i);
            order.swap(i, j);
        }
        for &idx in &order {
            let var = &var_names[idx];
            // Skip evidence-locked variables
            if evidence.contains_key(var) {
                continue;
            }
            // Compute P(var = true | current others)
            let mut assign_true = assignment.clone();
            assign_true.insert(var.clone(), true);
            let p_true = joint_log_prob(bn, &assign_true).exp();

            let mut assign_false = assignment.clone();
            assign_false.insert(var.clone(), false);
            let p_false = joint_log_prob(bn, &assign_false).exp();

            let z = p_true + p_false;
            let p = if z > 0.0 { p_true / z } else { 0.5 };
            let u: f64 = rng.r#gen();
            let new_val = u < p;
            assignment.insert(var.clone(), new_val);
        }

        // Record sample after burn-in, respecting thinning
        if it >= config.burn_in && (it - config.burn_in) % config.thinning == 0 {
            all_samples.push(assignment.clone());
            record_count += 1;
            for v in &var_names {
                *counts.entry(v.clone()).or_insert(0) +=
                    if assignment.get(v).copied().unwrap_or(false) {
                        1
                    } else {
                        0
                    };
            }
        }
    }

    let marginals: HashMap<String, f64> = if record_count > 0 {
        counts
            .iter()
            .map(|(v, c)| (v.clone(), *c as f64 / record_count as f64))
            .collect()
    } else {
        HashMap::new()
    };

    GibbsResult {
        samples: all_samples,
        marginals,
        iterations: config.iterations,
    }
}

/// Joint log-probability of a full assignment under the BN.
///
/// For Bernoulli variables, `log p(assignment) = Σ_i log P(X_i = a_i | parents(a))`.
fn joint_log_prob(bn: &BayesianNetwork, assign: &HashMap<String, bool>) -> f64 {
    let mut log_p = 0.0;
    for cpt in &bn.nodes {
        let val = assign.get(&cpt.variable).copied().unwrap_or(false);
        let pt = conditional_prob(cpt, assign);
        let p = if val { pt } else { 1.0 - pt };
        if p > 0.0 {
            log_p += p.ln();
        } else {
            log_p += -700.0; // clamp to avoid -inf
        }
    }
    log_p
}

/// P(variable = true | current values of parents in `assign`).
fn conditional_prob(cpt: &Cpt, assign: &HashMap<String, bool>) -> f64 {
    let mut bits = 0usize;
    for (i, par) in cpt.parents.iter().enumerate() {
        if assign.get(par).copied().unwrap_or(false) {
            bits |= 1 << i;
        }
    }
    cpt.probs_true.get(bits).copied().unwrap_or(0.5)
}

/// Convenience: query P(var=true | evidence) using Gibbs sampling.
pub fn gibbs_query(
    bn: &BayesianNetwork,
    query: &str,
    evidence: &HashMap<String, bool>,
    config: &GibbsConfig,
    seed: u64,
) -> f64 {
    let result = gibbs_sample(bn, evidence, config, seed);
    result.marginals.get(query).copied().unwrap_or(0.5)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rain_sprinkler() -> BayesianNetwork {
        let mut bn = BayesianNetwork::new();
        bn.add_node(Cpt {
            variable: "Rain".into(),
            parents: vec![],
            probs_true: vec![0.2],
        });
        bn.add_node(Cpt {
            variable: "Sprinkler".into(),
            parents: vec![],
            probs_true: vec![0.1],
        });
        bn.add_node(Cpt {
            variable: "Wet".into(),
            parents: vec!["Rain".into(), "Sprinkler".into()],
            probs_true: vec![0.0, 0.9, 0.8, 0.99],
        });
        bn
    }

    #[test]
    fn gibbs_recovers_rain_prior() {
        let bn = rain_sprinkler();
        let config = GibbsConfig {
            iterations: 1500,
            burn_in: 300,
            thinning: 2,
        };
        let result = gibbs_sample(&bn, &HashMap::new(), &config, 42);
        let p_rain = result.marginals.get("Rain").copied().unwrap_or(0.0);
        // Prior P(Rain) = 0.2; sample estimate should be within 0.1
        assert!(
            (p_rain - 0.2).abs() < 0.1,
            "P(Rain) = {p_rain}, expected ~0.2"
        );
    }

    #[test]
    fn gibbs_query_with_evidence_increases_rain() {
        let bn = rain_sprinkler();
        let config = GibbsConfig {
            iterations: 1500,
            burn_in: 300,
            thinning: 2,
        };
        let mut ev = HashMap::new();
        ev.insert("Wet".into(), true);
        let p_rain = gibbs_query(&bn, "Rain", &ev, &config, 42);
        // P(Rain | Wet) ≈ 0.74; must be substantially above prior 0.2
        assert!(p_rain > 0.5, "P(Rain|Wet) ≈ 0.74, Gibbs got {p_rain}");
    }

    #[test]
    fn evidence_locked_variables_remain_fixed() {
        let bn = rain_sprinkler();
        let config = GibbsConfig {
            iterations: 200,
            burn_in: 50,
            thinning: 1,
        };
        let mut ev = HashMap::new();
        ev.insert("Rain".into(), true);
        let result = gibbs_sample(&bn, &ev, &config, 1);
        for sample in &result.samples {
            assert_eq!(sample.get("Rain").copied(), Some(true));
        }
    }

    #[test]
    fn marginals_within_zero_one() {
        let bn = rain_sprinkler();
        let config = GibbsConfig::default();
        let result = gibbs_sample(&bn, &HashMap::new(), &config, 7);
        for (v, p) in &result.marginals {
            assert!(*p >= 0.0 && *p <= 1.0, "{v}: {p}");
        }
    }

    #[test]
    fn empty_bn_returns_empty_marginals() {
        let bn = BayesianNetwork::new();
        let config = GibbsConfig::default();
        let result = gibbs_sample(&bn, &HashMap::new(), &config, 7);
        assert!(result.marginals.is_empty());
        assert!(result.samples.is_empty());
    }
}
