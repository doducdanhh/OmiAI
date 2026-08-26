//! Hamiltonian Monte Carlo (HMC) for continuous-variable Bayesian inference.
//!
//! HMC reduces random-walk correlation by simulating Hamiltonian dynamics
//! in an augmented (position, momentum) phase space. A Metropolis–Hastings
//! correction on the Hamiltonian ensures correctness, while proposals can
//! traverse the typical set in a single leapfrog trajectory.
//!
//! # Algorithm (Neal 2011, Betancourt 2017)
//!
//! 1. Sample momentum `p ~ N(0, M)` from a Gaussian (mass matrix `M`).
//! 2. Simulate Hamilton's equations for `L` leapfrog steps of size `ε`:
//!    `p ← p + (ε/2) · ∇logπ(q)`; `q ← q + ε · M⁻¹ · p`; `p ← p + (ε/2) · ∇logπ(q)`.
//! 3. Accept `(q', p')` with probability `min(1, exp(H(q,p) − H(q',p')))`.
//!
//! # Usage
//!
//! ```ignore
//! use omiai::probabilistic::hmc::{HmcSampler, LogDensity};
//!
//! struct UnitGaussian;
//! impl LogDensity for UnitGaussian {
//!     fn dim(&self) -> usize { 1 }
//!     fn log_density(&self, q: &[f64], grad: &mut [f64]) -> f64 {
//!         grad[0] = -q[0];
//!         -0.5 * q[0] * q[0]
//!     }
//! }
//!
//! let sampler = HmcSampler::new(1, 0.1, 20, 1000);
//! let samples = sampler.sample(&UnitGaussian, 7);
//! ```

use rand::Rng;
use rand::SeedableRng;
use rand::distributions::Distribution;
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;

/// A target log-density `log π(q)` with gradient `∇log π(q)`.
///
/// Implementors should write `grad[i] = ∂log π(q) / ∂q_i` and return
/// `log π(q)` (not normalized, since HMC only needs it up to a constant).
pub trait LogDensity {
    /// Number of continuous variables.
    fn dim(&self) -> usize;

    /// Evaluate `log π(q)` and write the gradient into `grad`.
    ///
    /// `grad` is pre-zeroed; the implementor should fill it entirely.
    fn log_density(&self, q: &[f64], grad: &mut [f64]) -> f64;
}

/// Hamiltonian Monte Carlo sampler configuration.
#[derive(Debug, Clone)]
pub struct HmcSampler {
    /// Step size ε for the leapfrog integrator.
    pub step_size: f64,
    /// Number of leapfrog steps per proposal (L).
    pub num_leapfrog: usize,
    /// Total number of iterations (including burn-in).
    pub num_samples: usize,
    /// Number of initial iterations discarded as burn-in.
    pub burn_in: usize,
}

impl HmcSampler {
    /// Construct with all parameters.
    pub fn new(step_size: f64, num_leapfrog: usize, num_samples: usize) -> Self {
        Self {
            step_size,
            num_leapfrog,
            num_samples,
            burn_in: num_samples / 2,
        }
    }

    /// Set burn-in explicitly.
    pub fn with_burn_in(mut self, burn_in: usize) -> Self {
        self.burn_in = burn_in;
        self
    }

    /// Run the sampler and return post-burn-in samples plus diagnostics.
    ///
    /// `seed` initializes the RNG for reproducibility.
    pub fn sample<D: LogDensity>(&self, density: &D, seed: u64) -> HmcResult {
        let dim = density.dim();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let q = vec![0.0f64; dim];
        let mut grad = vec![0.0f64; dim];
        let log_p_initial = density.log_density(&q, &mut grad);

        let mut samples: Vec<Vec<f64>> = Vec::with_capacity(self.num_samples);
        let mut accepted = 0usize;
        let mut divergences = 0usize;
        let mut energies: Vec<f64> = Vec::with_capacity(self.num_samples);

        // Iterate
        let mut q_cur = q.clone();
        let mut log_p_cur = log_p_initial;
        let mut grad_cur = grad.clone();

        for it in 0..self.num_samples {
            // 1. Sample momentum
            let p: Vec<f64> = (0..dim).map(|_| StandardNormal.sample(&mut rng)).collect();

            // 2. Simulate leapfrog
            let mut q_prop = q_cur.clone();
            let mut grad_prop = grad_cur.clone();
            let mut p_prop = p.clone();

            // Half-step for momentum
            for i in 0..dim {
                p_prop[i] += 0.5 * self.step_size * grad_prop[i];
            }
            // Full alternating steps
            for _step in 0..self.num_leapfrog {
                for i in 0..dim {
                    q_prop[i] += self.step_size * p_prop[i];
                }
                let log_p_prop = density.log_density(&q_prop, &mut grad_prop);
                if !log_p_prop.is_finite() {
                    divergences += 1;
                    break;
                }
                for i in 0..dim {
                    p_prop[i] += self.step_size * grad_prop[i];
                }
            }
            // Final half-step
            for i in 0..dim {
                p_prop[i] += 0.5 * self.step_size * grad_prop[i];
            }
            // Negate momentum to preserve reversibility
            for p_i in p_prop.iter_mut() {
                *p_i = -*p_i;
            }

            // 3. Metropolis acceptance
            let log_p_prop = density.log_density(&q_prop, &mut grad_prop);
            let kinetic_old: f64 = p.iter().map(|x| x * x).sum::<f64>() * 0.5;
            let kinetic_new: f64 = p_prop.iter().map(|x| x * x).sum::<f64>() * 0.5;
            let h_old = -(log_p_cur) + kinetic_old;
            let h_new = -(log_p_prop) + kinetic_new;
            let log_alpha = (h_old - h_new).min(0.0);
            let u: f64 = rng.r#gen::<f64>();
            if u.ln() < log_alpha {
                q_cur = q_prop;
                log_p_cur = log_p_prop;
                grad_cur = grad_prop;
                accepted += 1;
            }
            energies.push(h_old);

            // Only record after burn-in
            if it >= self.burn_in {
                samples.push(q_cur.clone());
            }
            let _ = log_p_initial;
        }

        let acceptance_rate = accepted as f64 / self.num_samples as f64;
        HmcResult {
            samples,
            acceptance_rate,
            divergences,
            energies,
        }
    }

    /// Adapt step size during a warm-up phase to target a desired
    /// acceptance rate (default 0.65 — optimal for HMC).
    pub fn adapt<D: LogDensity>(
        &mut self,
        density: &D,
        target_accept: f64,
        warmup: usize,
        seed: u64,
    ) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut q = vec![0.0f64; density.dim()];
        let mut grad = vec![0.0f64; density.dim()];
        let _ = density.log_density(&q, &mut grad);

        let mut log_step = self.step_size.ln();
        let log_step_min = (self.step_size * 0.01).ln();
        let log_step_max = (self.step_size * 100.0).ln();
        let gamma: f64 = 0.05;
        let kappa: f64 = 0.75;
        let t0: f64 = 10.0;

        for it in 0..warmup {
            // Simple HMC step
            let p: Vec<f64> = (0..density.dim())
                .map(|_| StandardNormal.sample(&mut rng))
                .collect();
            let mut q_prop = q.clone();
            let mut grad_prop = grad.clone();
            let mut p_prop = p.clone();
            for i in 0..density.dim() {
                p_prop[i] += 0.5 * self.step_size * grad_prop[i];
            }
            for _ in 0..self.num_leapfrog {
                for i in 0..density.dim() {
                    q_prop[i] += self.step_size * p_prop[i];
                }
                let _ = density.log_density(&q_prop, &mut grad_prop);
                for i in 0..density.dim() {
                    p_prop[i] += self.step_size * grad_prop[i];
                }
            }
            for i in 0..density.dim() {
                p_prop[i] += 0.5 * self.step_size * grad_prop[i];
            }
            for pi in p_prop.iter_mut() {
                *pi = -*pi;
            }
            let log_p_prop = density.log_density(&q_prop, &mut grad_prop);
            let log_p_cur = density.log_density(&q, &mut grad);
            let ko: f64 = p.iter().map(|x| x * x).sum::<f64>() * 0.5;
            let kn: f64 = p_prop.iter().map(|x| x * x).sum::<f64>() * 0.5;
            let accept =
                rng.r#gen::<f64>().ln() < ((-log_p_cur + ko) - (-log_p_prop + kn)).min(0.0);
            let m = it as f64 + 1.0;
            let _h = (1.0 - 1.0 / (m + t0)).mul_add(-target_accept, target_accept);
            let _log_alpha = ((-log_p_cur + ko) - (-log_p_prop + kn)).min(0.0);
            let z: f64 = if accept { 1.0 } else { 0.0 };
            // NUTS dual-averaging (Hoffman & Gelman 2014):
            //   log_step += gamma * (z - target_accept) / m^kappa
            // The accumulator `h` is used only to compute the final
            // averaged step size after warmup; do NOT add it here.
            log_step += gamma.mul_add(z - target_accept, 0.0) * (m).powf(-kappa);
            log_step = log_step.clamp(log_step_min, log_step_max);
            if accept {
                q = q_prop;
                grad = grad_prop;
            }
            self.step_size = log_step.exp();
        }
    }
}

/// Output of [`HmcSampler::sample`].
#[derive(Debug, Clone)]
pub struct HmcResult {
    /// Post-burn-in samples.
    pub samples: Vec<Vec<f64>>,
    /// Empirical Metropolis acceptance rate.
    pub acceptance_rate: f64,
    /// Number of iterations that diverged (infinite log-density).
    pub divergences: usize,
    /// Per-iteration Hamiltonian energies (full trace, including burn-in).
    pub energies: Vec<f64>,
}

impl HmcResult {
    /// Posterior mean across all sampled dimensions.
    pub fn mean(&self) -> Vec<f64> {
        if self.samples.is_empty() {
            return Vec::new();
        }
        let dim = self.samples[0].len();
        let n = self.samples.len() as f64;
        let mut m = vec![0.0f64; dim];
        for s in &self.samples {
            for (i, v) in s.iter().enumerate() {
                m[i] += v / n;
            }
        }
        m
    }

    /// Posterior standard deviation per dimension.
    pub fn std_dev(&self) -> Vec<f64> {
        if self.samples.is_empty() {
            return Vec::new();
        }
        let dim = self.samples[0].len();
        let m = self.mean();
        let n = self.samples.len() as f64;
        let mut v = vec![0.0f64; dim];
        for s in &self.samples {
            for (i, x) in s.iter().enumerate() {
                let d = x - m[i];
                v[i] += d * d / n;
            }
        }
        v.into_iter().map(|x| x.sqrt()).collect()
    }
}

// ---------------------------------------------------------------------------
// Common densities
// ---------------------------------------------------------------------------

/// Standard multivariate normal `N(0, I)` log density.
pub struct StandardNormal_(pub usize);

impl LogDensity for StandardNormal_ {
    fn dim(&self) -> usize {
        self.0
    }
    fn log_density(&self, q: &[f64], grad: &mut [f64]) -> f64 {
        let mut lp = 0.0;
        for (i, &qi) in q.iter().enumerate() {
            grad[i] = -qi;
            lp -= 0.5 * qi * qi;
        }
        lp
    }
}

/// Multivariate normal with fixed mean μ and covariance σ²·I.
pub struct IsotropicNormal {
    pub dim: usize,
    pub mean: Vec<f64>,
    pub std: f64,
}

impl LogDensity for IsotropicNormal {
    fn dim(&self) -> usize {
        self.dim
    }
    fn log_density(&self, q: &[f64], grad: &mut [f64]) -> f64 {
        let inv_var = 1.0 / (self.std * self.std);
        let mut lp = 0.0;
        for (i, &qi) in q.iter().enumerate() {
            let mu = self.mean.get(i).copied().unwrap_or(0.0);
            let d = qi - mu;
            grad[i] = -inv_var * d;
            lp -= 0.5 * inv_var * d * d;
        }
        lp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_unit_gaussian_close_to_zero_mean() {
        let density = StandardNormal_(2);
        let sampler = HmcSampler::new(0.5, 20, 1000);
        let result = sampler.sample(&density, 42);
        let m = result.mean();
        assert_eq!(m.len(), 2);
        for &v in &m {
            assert!(v.abs() < 0.3, "mean drifted too far: {v}");
        }
        assert!(result.acceptance_rate > 0.5);
    }

    #[test]
    fn samples_isotropic_normal_recovers_mean() {
        let density = IsotropicNormal {
            dim: 1,
            mean: vec![3.0],
            std: 1.5,
        };
        let sampler = HmcSampler::new(0.4, 25, 800);
        let result = sampler.sample(&density, 7);
        let m = result.mean();
        assert!((m[0] - 3.0).abs() < 0.3, "mean={} expected ~3.0", m[0]);
    }

    #[test]
    fn std_dev_positive() {
        let density = StandardNormal_(3);
        let sampler = HmcSampler::new(0.3, 20, 600);
        let result = sampler.sample(&density, 99);
        let sd = result.std_dev();
        for &v in &sd {
            assert!(v > 0.5 && v < 2.0, "std_dev out of range: {v}");
        }
    }

    #[test]
    fn divergence_count_is_zero_for_smooth_density() {
        let density = StandardNormal_(2);
        let sampler = HmcSampler::new(0.5, 20, 500);
        let result = sampler.sample(&density, 1);
        assert_eq!(result.divergences, 0);
    }
}
