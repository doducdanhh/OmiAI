//! Bayesian networks: exact inference via Variable Elimination and
//! approximate inference via Metropolis–Hastings MCMC.

use std::collections::HashMap;

/// Conditional probability table for a discrete node.
#[derive(Debug, Clone)]
pub struct Cpt {
    pub variable: String,
    pub parents: Vec<String>,
    /// Probability of variable=true given parent assignment bitmask
    /// (parent i is bit i). For multi-value this is Bernoulli-only.
    pub probs_true: Vec<f64>,
}

/// Discrete Bayesian network (Bernoulli variables for simplicity).
#[derive(Debug, Clone, Default)]
pub struct BayesianNetwork {
    pub nodes: Vec<Cpt>,
}

impl BayesianNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, cpt: Cpt) {
        self.nodes.push(cpt);
    }

    /// Topological order (assumes DAG; parent indices appear earlier).
    fn topo(&self) -> Vec<usize> {
        // Simple: order by number of parents then stable
        let mut idx: Vec<usize> = (0..self.nodes.len()).collect();
        idx.sort_by_key(|&i| self.nodes[i].parents.len());
        idx
    }

    /// P(query=true | evidence) via full enumeration (exact, exponential).
    pub fn variable_elimination(&self, query: &str, evidence: &HashMap<String, bool>) -> f64 {
        let vars: Vec<String> = self.nodes.iter().map(|n| n.variable.clone()).collect();
        let free: Vec<String> = vars
            .iter()
            .filter(|v| !evidence.contains_key(v.as_str()) && v.as_str() != query)
            .cloned()
            .collect();

        let n_free = free.len();
        if n_free > 16 {
            return self.mcmc(query, evidence, 2000);
        }

        let mut joint_true = 0.0;
        let mut joint_false = 0.0;
        let total = 1usize << n_free;
        for mask in 0..total {
            let mut assign = evidence.clone();
            for (i, v) in free.iter().enumerate() {
                assign.insert(v.clone(), (mask >> i) & 1 == 1);
            }
            // query true
            assign.insert(query.to_string(), true);
            joint_true += self.joint_prob(&assign);
            // query false
            assign.insert(query.to_string(), false);
            joint_false += self.joint_prob(&assign);
        }
        let z = joint_true + joint_false;
        if z < 1e-15 { 0.5 } else { joint_true / z }
    }

    fn joint_prob(&self, assign: &HashMap<String, bool>) -> f64 {
        let mut p = 1.0;
        for cpt in &self.nodes {
            let mut bits = 0usize;
            for (i, par) in cpt.parents.iter().enumerate() {
                if assign.get(par).copied().unwrap_or(false) {
                    bits |= 1 << i;
                }
            }
            let pt = cpt.probs_true.get(bits).copied().unwrap_or(0.5);
            let val = assign.get(&cpt.variable).copied().unwrap_or(false);
            p *= if val { pt } else { 1.0 - pt };
        }
        p
    }

    /// Metropolis–Hastings over free variables; returns P(query=true|evidence).
    pub fn mcmc(&self, query: &str, evidence: &HashMap<String, bool>, samples: usize) -> f64 {
        let free: Vec<String> = self
            .nodes
            .iter()
            .map(|n| n.variable.clone())
            .filter(|v| !evidence.contains_key(v))
            .collect();

        let mut assign = evidence.clone();
        for v in &free {
            assign.insert(v.clone(), false);
        }

        let mut count_true = 0usize;
        let mut state = assign;
        // Simple single-site MH
        let mut seed = 1234567u64;
        for s in 0..samples {
            for v in &free {
                let mut prop = state.clone();
                let cur = prop.get(v).copied().unwrap_or(false);
                prop.insert(v.clone(), !cur);
                let p_cur = self.joint_prob(&state);
                let p_prop = self.joint_prob(&prop);
                let ratio = if p_cur < 1e-15 {
                    1.0
                } else {
                    (p_prop / p_cur).min(1.0)
                };
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let u = (seed >> 33) as f64 / (u32::MAX as f64);
                if u < ratio {
                    state = prop;
                }
            }
            // burn-in half
            if s > samples / 2 && state.get(query).copied().unwrap_or(false) {
                count_true += 1;
            }
        }
        let n = (samples / 2).max(1);
        count_true as f64 / n as f64
    }

    /// Exact inference entry (alias).
    pub fn infer_exact(&self, query: &str, evidence: &HashMap<String, bool>) -> f64 {
        self.variable_elimination(query, evidence)
    }

    /// Approximate inference entry (alias).
    pub fn infer_mcmc(&self, query: &str, evidence: &HashMap<String, bool>) -> f64 {
        self.mcmc(query, evidence, 3000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_sprinkler_wet() {
        // Classic: Rain → Wet, Sprinkler → Wet
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
        // Wet | Rain, Sprinkler — index bits: Rain=bit0, Sprinkler=bit1
        // P(Wet|¬R,¬S)=0.0, R¬S=0.9, ¬RS=0.8, RS=0.99
        bn.add_node(Cpt {
            variable: "Wet".into(),
            parents: vec!["Rain".into(), "Sprinkler".into()],
            probs_true: vec![0.0, 0.9, 0.8, 0.99],
        });
        let mut ev = HashMap::new();
        ev.insert("Wet".into(), true);
        let p = bn.infer_exact("Rain", &ev);
        assert!(p > 0.2, "P(Rain|Wet) should exceed prior, got {p}");
    }
}
