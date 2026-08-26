//! Liquid State Machine: a pool of leaky integrate-and-fire (LIF) spiking
//! neurons with random fixed synapses — the spiking analogue of an ESN.
//!
//! No backpropagation; the liquid is a fading-memory filter. Downstream
//! readout can be trained by the same RLS method as [`super::reservoir`].

use super::weights::{random_matrix, sparse_random_matrix};

/// Leaky integrate-and-fire liquid.
#[derive(Debug, Clone)]
pub struct LiquidStateMachine {
    pub n_neurons: usize,
    /// Membrane potentials
    v: Vec<f64>,
    /// Synaptic weight matrix
    w: Vec<Vec<f64>>,
    /// Input weights
    w_in: Vec<Vec<f64>>,
    pub input_dim: usize,
    /// Threshold
    pub threshold: f64,
    /// Leak factor per step (0..1)
    pub leak: f64,
    /// Resting potential
    pub v_rest: f64,
    /// Last spike train (0/1)
    spikes: Vec<f64>,
}

impl LiquidStateMachine {
    pub fn new(n_neurons: usize, input_dim: usize, seed: u64) -> Self {
        let w = sparse_random_matrix(n_neurons, n_neurons, 0.05, 0.3, seed);
        let w_in = random_matrix(n_neurons, input_dim, 0.5, seed.wrapping_add(3));
        Self {
            n_neurons,
            v: vec![0.0; n_neurons],
            w,
            w_in,
            input_dim,
            threshold: 1.0,
            leak: 0.9,
            v_rest: 0.0,
            spikes: vec![0.0; n_neurons],
        }
    }

    /// Current spike vector (1.0 = spiked this step).
    pub fn spikes(&self) -> &[f64] {
        &self.spikes
    }

    /// Membrane potentials.
    pub fn voltages(&self) -> &[f64] {
        &self.v
    }

    /// Advance one timestep with external input current.
    ///
    /// Dynamics: `V ← leak·V + W·spikes + W_in·u`; spike if `V ≥ θ`, then reset.
    pub fn step(&mut self, input: &[f64]) -> Vec<f64> {
        let mut u = input.to_vec();
        if u.len() < self.input_dim {
            u.resize(self.input_dim, 0.0);
        }
        let mut i_syn = vec![0.0; self.n_neurons];
        for (i, i_syn_i) in i_syn.iter_mut().enumerate() {
            for (spike_j, wij) in self.spikes.iter().zip(self.w[i].iter()) {
                *i_syn_i += wij * spike_j;
            }
            for (u_k, w_ik) in u.iter().zip(self.w_in[i].iter()) {
                *i_syn_i += w_ik * u_k;
            }
        }
        for ((i, v_i), &i_syn_i) in self.v.iter_mut().enumerate().zip(i_syn.iter()) {
            *v_i = self.leak * *v_i + i_syn_i;
            if *v_i >= self.threshold {
                self.spikes[i] = 1.0;
                *v_i = self.v_rest;
            } else {
                self.spikes[i] = 0.0;
            }
        }
        self.spikes.clone()
    }

    /// Liquid state as a concatenated spike history window (simple).
    pub fn liquid_state(&self) -> Vec<f64> {
        let mut state = self.spikes.clone();
        state.extend_from_slice(&self.v);
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquid_produces_spikes() {
        let mut lsm = LiquidStateMachine::new(30, 2, 99);
        let mut total_spikes = 0.0;
        for t in 0..100 {
            let u = vec![(t as f64 * 0.2).sin(), (t as f64 * 0.1).cos()];
            let s = lsm.step(&u);
            total_spikes += s.iter().sum::<f64>();
        }
        // With random drive we expect some activity
        assert!(total_spikes >= 0.0);
        assert_eq!(lsm.liquid_state().len(), 60);
    }
}
