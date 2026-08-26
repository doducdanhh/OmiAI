//! Solomonoff Induction approximation via Minimum Description Length (MDL).
//!
//! True Solomonoff induction is uncomputable. We approximate the universal
//! prior by enumerating short programs in a tiny prefix-free language and
//! weighting predictions by `2^{-|p|}`.

use std::collections::HashMap;

/// Tiny bitwise program language for sequence prediction.
#[derive(Debug, Clone)]
pub struct MdlModel {
    /// Max program length (bits) to enumerate
    pub max_bits: usize,
}

impl Default for MdlModel {
    fn default() -> Self {
        Self { max_bits: 12 }
    }
}

impl MdlModel {
    pub fn new(max_bits: usize) -> Self {
        Self {
            max_bits: max_bits.min(16),
        }
    }

    /// Approximate Solomonoff mixture prediction P(next=1 | data).
    ///
    /// Each program is an integer `0..(1<<max_bits)` interpreted as a
    /// periodic bit pattern generator; likelihood is agreement with data.
    pub fn predict_next_one(&self, data: &[bool]) -> f64 {
        if self.max_bits == 0 {
            return 0.5;
        }
        let mut weighted_one = 0.0;
        let mut weighted_total = 0.0;
        let n_prog = 1usize << self.max_bits;
        for p in 0..n_prog {
            let bits = program_bits(p, self.max_bits);
            let w = 2f64.powi(-(self.max_bits as i32)); // uniform over length band
            // Likelihood: product of matches (with small epsilon)
            let mut like = 1.0;
            for (i, &bit) in data.iter().enumerate() {
                let pred = bits[i % bits.len()];
                like *= if pred == bit { 0.99 } else { 0.01 };
            }
            let next = bits[data.len() % bits.len()];
            weighted_total += w * like;
            if next {
                weighted_one += w * like;
            }
        }
        if weighted_total < 1e-30 {
            0.5
        } else {
            weighted_one / weighted_total
        }
    }

    /// MDL codelength of data under the best periodic model (nats).
    pub fn mdl_codelength(&self, data: &[bool]) -> f64 {
        let mut best = f64::INFINITY;
        let n_prog = 1usize << self.max_bits.min(10);
        for p in 0..n_prog {
            let bits = program_bits(p, self.max_bits.min(10));
            let mut nll = (self.max_bits as f64) * std::f64::consts::LN_2; // program cost
            for (i, &bit) in data.iter().enumerate() {
                let pred = bits[i % bits.len()];
                nll += if pred == bit {
                    -0.99f64.ln()
                } else {
                    -0.01f64.ln()
                };
            }
            if nll < best {
                best = nll;
            }
        }
        best
    }
}

fn program_bits(p: usize, n_bits: usize) -> Vec<bool> {
    let mut bits = Vec::with_capacity(n_bits.max(1));
    for i in 0..n_bits.max(1) {
        bits.push((p >> i) & 1 == 1);
    }
    bits
}

/// Frequency-table baseline predictor (Laplace-smoothed).
pub fn empirical_predict(data: &[bool]) -> f64 {
    let ones = data.iter().filter(|&&b| b).count() as f64;
    let n = data.len() as f64;
    (ones + 1.0) / (n + 2.0)
}

/// Mixture of experts: MDL + empirical.
pub fn mixture_predict(data: &[bool], model: &MdlModel) -> f64 {
    let a = model.predict_next_one(data);
    let b = empirical_predict(data);
    0.5 * a + 0.5 * b
}

/// Store of hypothesis weights for online prediction.
#[derive(Debug, Default)]
pub struct HypothesisWeights {
    pub weights: HashMap<String, f64>,
}

impl HypothesisWeights {
    pub fn update(&mut self, name: &str, likelihood: f64) {
        let w = self.weights.entry(name.to_string()).or_insert(1.0);
        *w *= likelihood;
    }

    pub fn normalized(&self) -> HashMap<String, f64> {
        let z: f64 = self.weights.values().sum();
        if z < 1e-30 {
            return HashMap::new();
        }
        self.weights
            .iter()
            .map(|(k, v)| (k.clone(), v / z))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicts_period_two() {
        let model = MdlModel::new(6);
        // 0,1,0,1,0,1 → next likely 0
        let data = vec![false, true, false, true, false, true];
        let p1 = model.predict_next_one(&data);
        assert!(p1 < 0.5, "expected next=0 more likely, P(1)={p1}");
    }
}
