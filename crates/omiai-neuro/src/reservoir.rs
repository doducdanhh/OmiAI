//! Criticality-tuned Echo State Network (ESN) reservoir.
//!
//! Fixed random recurrent weights (never backpropagated), spectral radius
//! near the edge of chaos, readout trained by Recursive Least Squares
//! (RLS) / ridge regression. Lyapunov exponent estimation monitors
//! criticality.
//!
//! # References
//! - Jaeger, *The "echo state" approach to analysing and training RNNs*
//! - Sussillo & Abbott, *Generating Coherent Patterns of Activity* (FORCE)

use super::weights::{matvec, normalize_spectral_radius, random_matrix, sparse_random_matrix};

/// Echo State Network reservoir with linear readout.
#[derive(Debug, Clone)]
pub struct Reservoir {
    pub size: usize,
    pub spectral_radius: f64,
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
    /// Recurrent weights (size × size)
    w: Vec<Vec<f64>>,
    /// Input weights (size × input_dim)
    w_in: Vec<Vec<f64>>,
    /// Readout weights (output_dim × size)
    w_out: Vec<Vec<f64>>,
    /// Current reservoir state
    state: Vec<f64>,
    /// RLS inverse correlation matrix (size × size)
    p_matrix: Vec<Vec<f64>>,
    leak_rate: f64,
}

impl Reservoir {
    /// Construct a reservoir with random weights, spectral-radius normalized.
    ///
    /// - `size`: number of reservoir units
    /// - `input_dim` / `output_dim`: I/O dimensions
    /// - `spectral_radius`: typically 0.9–1.0 for edge-of-chaos
    /// - `seed`: RNG seed for reproducibility
    pub fn new(
        size: usize,
        input_dim: usize,
        output_dim: usize,
        spectral_radius: f64,
        seed: u64,
    ) -> Self {
        let mut w = sparse_random_matrix(size, size, 0.1, 1.0, seed);
        normalize_spectral_radius(&mut w, spectral_radius);
        let w_in = random_matrix(size, input_dim, 0.5, seed.wrapping_add(1));
        let w_out = vec![vec![0.0; size]; output_dim];
        // P = (1/δ) I for RLS
        let delta = 1.0;
        let mut p_matrix = vec![vec![0.0; size]; size];
        for (i, row) in p_matrix.iter_mut().enumerate() {
            row[i] = 1.0 / delta;
        }
        Self {
            size,
            spectral_radius,
            input_dim,
            output_dim,
            w,
            w_in,
            w_out,
            state: vec![0.0; size],
            p_matrix,
            leak_rate: 0.3,
        }
    }

    /// Convenience constructor matching the original scaffold signature.
    pub fn with_radius(size: usize, spectral_radius: f64) -> Self {
        Self::new(size, 1, 1, spectral_radius, 42)
    }

    /// Current reservoir state.
    pub fn state(&self) -> &[f64] {
        &self.state
    }

    /// Advance the reservoir by one timestep:  
    /// `x ← (1-α)x + α tanh(W x + W_in u)`
    pub fn step(&mut self, input: &[f64]) -> Vec<f64> {
        let mut u = input.to_vec();
        if u.len() < self.input_dim {
            u.resize(self.input_dim, 0.0);
        }
        let rec = matvec(&self.w, &self.state);
        let inp = matvec(&self.w_in, &u[..self.input_dim]);
        let alpha = self.leak_rate;
        for (i, st) in self.state.iter_mut().enumerate() {
            let pre = rec[i] + inp.get(i).copied().unwrap_or(0.0);
            *st = (1.0 - alpha) * *st + alpha * pre.tanh();
        }
        self.readout()
    }

    /// Linear readout `y = W_out x`.
    pub fn readout(&self) -> Vec<f64> {
        self.w_out
            .iter()
            .map(|row| row.iter().zip(self.state.iter()).map(|(w, x)| w * x).sum())
            .collect()
    }

    /// Train readout via RLS (Recursive Least Squares) over a sequence.
    ///
    /// For each timestep: step reservoir, then update `W_out` toward target.
    pub fn train_readout(&mut self, inputs: &[Vec<f64>], targets: &[Vec<f64>]) {
        assert_eq!(inputs.len(), targets.len());
        for (input, target) in inputs.iter().zip(targets.iter()) {
            self.step(input);
            self.rls_update(target);
        }
    }

    /// Single RLS update against target `y_d`.
    fn rls_update(&mut self, target: &[f64]) {
        let x = &self.state;
        let n = self.size;
        // k = P x / (1 + x^T P x)
        let mut px = vec![0.0; n];
        for (p_i, row) in px.iter_mut().zip(self.p_matrix.iter()) {
            for (xj, pij) in x.iter().zip(row.iter()) {
                *p_i += pij * xj;
            }
        }
        let denom: f64 = 1.0 + x.iter().zip(px.iter()).map(|(xi, pi)| xi * pi).sum::<f64>();
        let k: Vec<f64> = px.iter().map(|pi| pi / denom).collect();

        // error e = y_d - W_out x
        let y = self.readout();
        for (o, w_row) in self.w_out.iter_mut().enumerate() {
            let yd = target.get(o).copied().unwrap_or(0.0);
            let e = yd - y.get(o).copied().unwrap_or(0.0);
            for (kj, w) in k.iter().zip(w_row.iter_mut()) {
                *w += e * kj;
            }
        }

        // P ← P - k (x^T P)
        // first compute x^T P
        let mut xtp = vec![0.0; n];
        for (j, xtp_j) in xtp.iter_mut().enumerate() {
            for (xi, row) in x.iter().zip(self.p_matrix.iter()) {
                *xtp_j += xi * row[j];
            }
        }
        for ((k_i, row), xtp_j) in k.iter().zip(self.p_matrix.iter_mut()).zip(xtp.iter()) {
            for w in row.iter_mut() {
                *w -= k_i * xtp_j;
            }
        }
    }

    /// Ridge-regression batch readout (closed form) alternative.
    pub fn train_readout_ridge(&mut self, states: &[Vec<f64>], targets: &[Vec<f64>], ridge: f64) {
        let n = self.size;
        let t = states.len();
        if t == 0 {
            return;
        }
        // X^T X + λI  and  X^T Y
        let mut xtx = vec![vec![0.0; n]; n];
        for (i, row) in xtx.iter_mut().enumerate() {
            row[i] = ridge;
        }
        let out_dim = self.output_dim;
        let mut xty = vec![vec![0.0; out_dim]; n];

        for (s, y) in states.iter().zip(targets.iter()) {
            for (i, xtx_row) in xtx.iter_mut().enumerate() {
                for (j, cell) in xtx_row.iter_mut().enumerate() {
                    *cell += s[i] * s[j];
                }
                for (o, cell) in xty[i].iter_mut().enumerate() {
                    *cell += s[i] * y.get(o).copied().unwrap_or(0.0);
                }
            }
        }
        // Solve via Gauss-Jordan (small n)
        if let Some(inv) = invert_matrix(&xtx) {
            for (o, w_row) in self.w_out.iter_mut().enumerate() {
                for (j, w) in w_row.iter_mut().enumerate() {
                    let mut sum = 0.0;
                    for (k, inv_jk) in inv[j].iter().enumerate() {
                        sum += inv_jk * xty[k][o];
                    }
                    *w = sum;
                }
            }
        }
    }

    /// Estimate largest Lyapunov exponent via trajectory divergence.
    ///
    /// Positive ⇒ chaotic; near zero ⇒ edge of chaos; negative ⇒ stable.
    pub fn largest_lyapunov_exponent(&self) -> f64 {
        let mut r1 = self.clone();
        let mut r2 = self.clone();
        // Perturb r2
        if !r2.state.is_empty() {
            r2.state[0] += 1e-8;
        }
        let steps = 200;
        let mut sum_log = 0.0;
        let input = vec![0.1; self.input_dim.max(1)];
        for _ in 0..steps {
            r1.step(&input);
            r2.step(&input);
            let mut d2 = 0.0;
            for i in 0..self.size {
                let d = r1.state[i] - r2.state[i];
                d2 += d * d;
            }
            let d = d2.sqrt().max(1e-15);
            sum_log += d.ln();
            // Renormalize perturbation
            let scale = 1e-8 / d;
            for i in 0..self.size {
                r2.state[i] = r1.state[i] + (r2.state[i] - r1.state[i]) * scale;
            }
        }
        sum_log / steps as f64
    }
}

fn invert_matrix(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m = a.to_vec();
    let mut inv = vec![vec![0.0; n]; n];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..n {
        // Pivot
        let mut pivot = col;
        for r in col..n {
            if m[r][col].abs() > m[pivot][col].abs() {
                pivot = r;
            }
        }
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        inv.swap(col, pivot);
        let div = m[col][col];
        for j in 0..n {
            m[col][j] /= div;
            inv[col][j] /= div;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let factor = m[r][col];
            for j in 0..n {
                m[r][j] -= factor * m[col][j];
                inv[r][j] -= factor * inv[col][j];
            }
        }
    }
    Some(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservoir_steps_and_trains() {
        let mut r = Reservoir::new(20, 1, 1, 0.9, 7);
        let inputs: Vec<Vec<f64>> = (0..50).map(|t| vec![(t as f64 * 0.1).sin()]).collect();
        let targets: Vec<Vec<f64>> = inputs.iter().map(|u| vec![u[0] * 0.5]).collect();
        r.train_readout(&inputs, &targets);
        let out = r.step(&[0.0]);
        assert_eq!(out.len(), 1);
        let lyap = r.largest_lyapunov_exponent();
        assert!(lyap.is_finite());
    }
}
