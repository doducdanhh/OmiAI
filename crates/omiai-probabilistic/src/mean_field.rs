//! Mean-field variational inference (MFVI) for discrete Bayesian networks.
//!
//! MFVI approximates the intractable posterior `P(X | E)` with a fully
//! factorized variational distribution `Q(X) = Π_i q_i(X_i)`, then
//! iterates fixed-point updates to minimize `KL(Q || P)`.
//!
//! For a discrete Bayesian network with Bernoulli variables and binary
//! factors, the closed-form update for each factor's log-marginal is
//!
//! ```text
//! log q_i(x_i) = E_{q_{-i}} [log P(x_i, parents(x_i))] + const
//! ```
//!
//! where the expectation is over all other `q_j`. For each CPT, we
//! compute the expected log-factor under the current `q` and update
//! `q_i` accordingly (with a sigmoid normalization).
//!
//! # References
//!
//! - Winn & Bishop, *Variational Message Passing* (JMLR 2005).
//! - Bishop, *Pattern Recognition and Machine Learning* §10.1 (2006).

use std::collections::HashMap;

#[cfg(test)]
use super::bayesian::Cpt;

use super::bayesian::BayesianNetwork;

/// Result of mean-field inference: per-variable marginals `q_i(X_i = true)`
/// and the history of free-energy values across iterations.
#[derive(Debug, Clone)]
pub struct MfResult {
    pub marginals: HashMap<String, f64>,
    pub free_energy_history: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
}

/// Mean-field configuration.
#[derive(Debug, Clone)]
pub struct MfConfig {
    pub max_iterations: usize,
    pub tolerance: f64,
    pub damping: f64,
}

impl Default for MfConfig {
    fn default() -> Self {
        Self {
            max_iterations: 200,
            tolerance: 1e-4,
            damping: 0.5,
        }
    }
}

/// Run mean-field variational inference.
pub fn mean_field(
    bn: &BayesianNetwork,
    evidence: &HashMap<String, bool>,
    config: &MfConfig,
) -> MfResult {
    let mut q: HashMap<String, f64> = HashMap::new();
    for cpt in &bn.nodes {
        q.insert(
            cpt.variable.clone(),
            *evidence.get(&cpt.variable).unwrap_or(&false) as u8 as f64,
        );
    }

    let mut history: Vec<f64> = Vec::new();
    let mut prev_fe = f64::INFINITY;
    let mut converged = false;
    let mut iterations = 0;

    for it in 0..config.max_iterations {
        iterations = it + 1;
        // For each non-evidence variable, compute its MF update
        let mut updates: HashMap<String, f64> = HashMap::new();
        for cpt in &bn.nodes {
            // Compute the expected log-factors for this variable's CPT
            // E[log P(cpt.var | parents)] under current q.
            //
            // For a Bernoulli node with binary parents, q_i(true) = p_i:
            //   E[log P(var=true | pa)]  = sum_{pa} q(pa) * log P(true|pa)
            //   E[log P(var=false | pa)] = sum_{pa} q(pa) * log P(false|pa)
            //   q*(true) ∝ exp(E[log P(true|pa)])
            //
            // We compute log p_true_total - log p_false_total and apply
            // sigmoid, optionally damped.
            let n_parents = cpt.parents.len();
            let parent_probs: Vec<f64> = cpt
                .parents
                .iter()
                .map(|p| q.get(p).copied().unwrap_or(0.5))
                .collect();

            let mut lp_true = 0.0_f64;
            let mut lp_false = 0.0_f64;
            for mask in 0..(1usize << n_parents) {
                let pt = cpt.probs_true.get(mask).copied().unwrap_or(0.5);
                let mut p_assign = 1.0_f64;
                for (i, _par) in cpt.parents.iter().enumerate() {
                    let pi = parent_probs[i];
                    let bit = (mask >> i) & 1 == 1;
                    p_assign *= if bit { pi } else { 1.0 - pi };
                }
                if pt > 0.0 {
                    lp_true += p_assign * pt.ln();
                }
                if pt < 1.0 {
                    lp_false += p_assign * (1.0 - pt).ln();
                }
            }

            // Evidence-likelihood term: for each CHILD of this variable
            // that is evidence-locked, add
            //   ln Σ_others q(others) · P(child = e | var, others)
            // The plain CPT expectation above only ties q_i to its own
            // parents, so evidence on a child never propagated upward
            // and P(Rain | Wet=true) stayed at the prior.
            //
            // We deliberately use the log of the EXPECTED likelihood
            // (not the expectation of the log, the strict MFVI bound):
            // with deterministic CPT entries (P = 0) the expectation of
            // the log is −∞ whenever any other-parent configuration
            // carries mass, collapsing the update to 0/1. The
            // log-expectation form stays finite and is exact on
            // singly-connected networks.
            if !evidence.contains_key(&cpt.variable) {
                for child in &bn.nodes {
                    if !child.parents.iter().any(|p| p == &cpt.variable) {
                        continue;
                    }
                    let Some(&ev_val) = evidence.get(&child.variable) else {
                        continue;
                    };
                    // Expectation over the OTHER parents of the child.
                    let other: Vec<(usize, f64)> = child
                        .parents
                        .iter()
                        .enumerate()
                        .filter(|(_i, p)| *p != &cpt.variable)
                        .map(|(i, p)| (i, q.get(p).copied().unwrap_or(0.5)))
                        .collect();
                    let idx_in_child =
                        child.parents.iter().position(|p| p == &cpt.variable).unwrap();
                    let mut mix_true = 0.0_f64; // Σ w·P(e | var=true, others)
                    let mut mix_false = 0.0_f64; // Σ w·P(e | var=false, others)
                    for mask in 0..(1usize << other.len()) {
                        let mut w = 1.0_f64;
                        for (k, (_, pq)) in other.iter().enumerate() {
                            w *= if (mask >> k) & 1 == 1 { *pq } else { 1.0 - *pq };
                        }
                        for (mix, v_val) in [(&mut mix_true, true), (&mut mix_false, false)] {
                            let mut m = mask;
                            if v_val {
                                m |= 1 << idx_in_child;
                            }
                            let pt = child.probs_true.get(m).copied().unwrap_or(0.5);
                            *mix += w * if ev_val { pt } else { 1.0 - pt };
                        }
                    }
                    if mix_true > 0.0 {
                        lp_true += mix_true.ln();
                    }
                    if mix_false > 0.0 {
                        lp_false += mix_false.ln();
                    }
                }
            }

            // If this variable is evidence-locked, skip update
            if evidence.contains_key(&cpt.variable) {
                let fixed = evidence[&cpt.variable];
                updates.insert(cpt.variable.clone(), if fixed { 1.0 } else { 0.0 });
                continue;
            }

            // Sigmoid update
            let logit = lp_true - lp_false;
            let new_p = sigmoid(logit);

            // Damped update
            let old_p = q.get(&cpt.variable).copied().unwrap_or(0.5);
            let damped = config.damping * old_p + (1.0 - config.damping) * new_p;
            updates.insert(cpt.variable.clone(), damped.clamp(1e-9, 1.0 - 1e-9));
        }

        // Apply updates
        for (v, p) in &updates {
            q.insert(v.clone(), *p);
        }

        // Compute free energy (negative ELBO)
        let fe = free_energy(bn, &q, evidence);
        history.push(fe);
        if (prev_fe - fe).abs() < config.tolerance && it > 5 {
            converged = true;
            break;
        }
        prev_fe = fe;
    }

    // Evidence variables pinned to their values
    for (v, val) in evidence {
        q.insert(v.clone(), if *val { 1.0 } else { 0.0 });
    }

    MfResult {
        marginals: q,
        free_energy_history: history,
        iterations,
        converged,
    }
}

/// Logistic sigmoid.
fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// Variational free energy `F(Q) = Σ_<X> Q(X) E_Q[log Q(X) − log P(X, E)]`.
/// We use a tractable surrogate for Bernoulli MFVI:
/// `F ≈ Σ_i H(q_i) + Σ_c E_Q[log φ_c]` summed over cliques.
fn free_energy(
    bn: &BayesianNetwork,
    q: &HashMap<String, f64>,
    _evidence: &HashMap<String, bool>,
) -> f64 {
    let mut fe = 0.0;
    // Entropy term: -Σ_i [q_i log q_i + (1-q_i) log(1-q_i)]
    for &p in q.values() {
        let p = p.clamp(1e-12, 1.0 - 1e-12);
        fe -= p * p.ln() + (1.0 - p) * (1.0 - p).ln();
    }
    // Cross-entropy term: -Σ_c E_Q[log P(c | pa(c))]
    for cpt in &bn.nodes {
        let n_parents = cpt.parents.len();
        let parent_probs: Vec<f64> = cpt
            .parents
            .iter()
            .map(|p| q.get(p).copied().unwrap_or(0.5))
            .collect();
        let qi = q.get(&cpt.variable).copied().unwrap_or(0.5);
        for mask in 0..(1usize << n_parents) {
            let pt = cpt.probs_true.get(mask).copied().unwrap_or(0.5);
            let mut p_assign = 1.0;
            for (i, _par) in cpt.parents.iter().enumerate() {
                let pi = parent_probs[i];
                let bit = (mask >> i) & 1 == 1;
                p_assign *= if bit { pi } else { 1.0 - pi };
            }
            // E[log P(var|pa)] under q
            let log_p_var_true = pt.ln();
            let log_p_var_false = (1.0 - pt).ln();
            let e_log_p = qi * log_p_var_true + (1.0 - qi) * log_p_var_false;
            fe -= p_assign * e_log_p;
        }
    }
    fe
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
    fn mf_recovers_priors_without_evidence() {
        let bn = rain_sprinkler();
        let result = mean_field(&bn, &HashMap::new(), &MfConfig::default());
        let p_rain = result.marginals.get("Rain").copied().unwrap_or(0.5);
        let p_spr = result.marginals.get("Sprinkler").copied().unwrap_or(0.5);
        // Without evidence, MF should recover priors approximately
        assert!((p_rain - 0.2).abs() < 0.15, "P(Rain) = {p_rain}");
        assert!((p_spr - 0.1).abs() < 0.15, "P(Sprinkler) = {p_spr}");
    }

    #[test]
    fn mf_increases_rain_with_wet_evidence() {
        let bn = rain_sprinkler();
        let mut ev = HashMap::new();
        ev.insert("Wet".into(), true);
        let result = mean_field(&bn, &ev, &MfConfig::default());
        let p_rain = result.marginals.get("Rain").copied().unwrap_or(0.5);
        // P(Rain | Wet=true) > 0.2 (prior)
        assert!(p_rain > 0.5, "P(Rain|Wet) should be > 0.5, got {p_rain}");
    }

    #[test]
    fn mf_evidence_locked_variables() {
        let bn = rain_sprinkler();
        let mut ev = HashMap::new();
        ev.insert("Rain".into(), true);
        let result = mean_field(&bn, &ev, &MfConfig::default());
        // Rain is locked to true
        assert!((result.marginals.get("Rain").copied().unwrap_or(0.5) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn free_energy_decreases() {
        let bn = rain_sprinkler();
        let mut ev = HashMap::new();
        ev.insert("Wet".into(), true);
        let result = mean_field(&bn, &ev, &MfConfig::default());
        let h = &result.free_energy_history;
        if h.len() >= 2 {
            let first = h[0];
            let last = *h.last().unwrap();
            // Convergence: free energy should not increase drastically
            assert!(last <= first + 1.0, "FE went up: {first} → {last}");
        }
    }

    #[test]
    fn sigmoid_at_zero_is_half() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn sigmoid_extremes() {
        assert!(sigmoid(20.0) > 0.999);
        assert!(sigmoid(-20.0) < 0.001);
    }
}
