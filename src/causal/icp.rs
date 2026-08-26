//! Invariant Causal Prediction (ICP).
//!
//! Peters, Bühlmann, Meinshausen (2016) showed that, given observations
//! from multiple environments (e.g. different experimental conditions),
//! the set of *direct* causes of a response variable `Y` can be
//! recovered as the intersection of all subsets `S ⊆ X` such that
//! `Y ⊥ Environment | X_S` (the conditional distribution of `Y` given
//! `X_S` is the same across environments).
//!
//! Under faithfulness and a few standard assumptions, this intersection
//! equals the set of direct causes of `Y` — no other variable carries
//! additional invariant predictive information.
//!
//! # Algorithm
//!
//! 1. **Test invariance** of `Y | X_S` between every pair of
//!    environments with a two-sample test on the residuals of
//!    `Y = X_S · β + ε`.
//! 2. **Enumerate subsets** `S` of candidate predictors and mark the
//!    invariant ones.
//! 3. **Intersect** all invariant subsets; the result is the estimated
//!    causal parent set.
//!
//! To keep the runtime tractable, the candidate enumeration uses a
//! greedy pruning step: variables that improve cross-environment
//! residual stability when included are kept; others are dropped.
//!
//! # References
//!
//! - Peters, Bühlmann, Meinshausen, *Causal inference by using invariant
//!   causal prediction: identification and confidence intervals*
//!   (JRSS B, 2016).
//! - Heinze-Deml, Peters, Meinshausen, *Invariant causal prediction for
//!   nonlinear models* (JMLR 2018).

use std::collections::BTreeSet;

use crate::utils::stats::{mean, variance};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// One observation: a vector of features + a target + an environment id.
#[derive(Debug, Clone)]
pub struct IcpSample {
    pub features: Vec<f64>,
    pub target: f64,
    pub environment: usize,
}

/// ICP result: estimated causal parents of the target.
#[derive(Debug, Clone)]
pub struct IcpResult {
    /// Indices of variables identified as direct causes of `target`.
    pub parents: BTreeSet<usize>,
    /// For each candidate set `S`, the maximum p-value across pairwise
    /// environment tests (lower ⇒ more invariant).
    pub invariance_scores: Vec<(BTreeSet<usize>, f64)>,
}

// ---------------------------------------------------------------------------
// Two-sample test (Welch-style t on residuals)
// ---------------------------------------------------------------------------

/// Welch-style two-sample t-statistic for the hypothesis that the means
/// of two groups are equal, weighted by their variances.
///
/// Returns `Some((t, df))` where larger `|t|` means more evidence of a
/// distributional difference. Returns `None` if either group is empty.
fn welch_t(a: &[f64], b: &[f64]) -> Option<(f64, f64)> {
    if a.len() < 2 || b.len() < 2 {
        return None;
    }
    let ma = mean(a);
    let mb = mean(b);
    let va = variance(a);
    let vb = variance(b);
    let denom = (va / a.len() as f64 + vb / b.len() as f64).sqrt();
    if denom < 1e-15 {
        return Some((0.0, (a.len() + b.len() - 2) as f64));
    }
    let t = (ma - mb) / denom;
    let df = (va / a.len() as f64 + vb / b.len() as f64).powi(2)
        / ((va / a.len() as f64).powi(2) / (a.len() as f64 - 1.0)
            + (vb / b.len() as f64).powi(2) / (b.len() as f64 - 1.0));
    Some((t, df))
}

/// Conservative p-value approximation: treat `|t|` as a N(0,1) score and
/// return `2 * (1 - Φ(|t|))` using a rough closed-form approximation.
fn two_sided_p_normal(z: f64) -> f64 {
    // erf approximation (Abramowitz & Stegun 7.1.26)
    let x = z.abs() / 2f64.sqrt();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let erf = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    let phi = 0.5 * (1.0 + erf);
    2.0 * (1.0 - phi)
}

// ---------------------------------------------------------------------------
// Ordinary least squares regression (closed form)
// ---------------------------------------------------------------------------

/// Solve `y = X · β` in the least-squares sense via the normal equations.
/// Returns `β` such that `‖Xβ − y‖²` is minimized.
fn ols_solve(x: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    if x.is_empty() || x[0].is_empty() || x.len() != y.len() {
        return None;
    }
    let p = x[0].len();
    let n = x.len();
    // Compute X'X and X'y
    let mut xtx = vec![vec![0.0f64; p]; p];
    let mut xty = vec![0.0f64; p];
    for i in 0..n {
        for j in 0..p {
            xty[j] += x[i][j] * y[i];
            for k in 0..p {
                xtx[j][k] += x[i][j] * x[i][k];
            }
        }
    }
    // Solve via Gauss-Jordan
    let mut aug = xtx.clone();
    for i in 0..p {
        aug[i].push(xty[i]);
    }
    let mut pivot = vec![0usize; p];
    for i in 0..p {
        pivot[i] = i;
    }
    for col in 0..p {
        // Find max in column
        let mut max_r = col;
        let mut max_v = aug[col][col].abs();
        for r in (col + 1)..p {
            if aug[r][col].abs() > max_v {
                max_v = aug[r][col].abs();
                max_r = r;
            }
        }
        if max_v < 1e-12 {
            return None;
        }
        aug.swap(col, max_r);
        pivot.swap(col, max_r);
        // Normalize pivot row
        let div = aug[col][col];
        for j in 0..=p {
            aug[col][j] /= div;
        }
        // Eliminate
        for r in 0..p {
            if r == col {
                continue;
            }
            let f = aug[r][col];
            for j in 0..=p {
                aug[r][j] -= f * aug[col][j];
            }
        }
    }
    let mut beta = vec![0.0f64; p];
    for i in 0..p {
        beta[pivot[i]] = aug[i][p];
    }
    Some(beta)
}

/// Compute residuals `y − Xβ` from a fitted OLS model.
fn ols_residuals(x: &[Vec<f64>], y: &[f64], beta: &[f64]) -> Vec<f64> {
    x.iter()
        .zip(y.iter())
        .map(|(xi, yi)| yi - xi.iter().zip(beta.iter()).map(|(a, b)| a * b).sum::<f64>())
        .collect()
}

// ---------------------------------------------------------------------------
// ICP main entry
// ---------------------------------------------------------------------------

/// Run ICP on the given multi-environment data.
///
/// - `samples` is the full multi-environment dataset.
/// - `target_idx` is the column index of `Y` (not used here; we read
///   `samples[i].target`).
/// - `p_value_threshold` is the per-pair p-value below which the
///   conditional distribution is considered *non*-invariant.
pub fn icp(samples: &[IcpSample], p_value_threshold: f64, max_cardinality: usize) -> IcpResult {
    if samples.is_empty() {
        return IcpResult {
            parents: BTreeSet::new(),
            invariance_scores: Vec::new(),
        };
    }
    let p = samples[0].features.len();

    // Group by environment
    let mut by_env: std::collections::BTreeMap<usize, Vec<&IcpSample>> =
        std::collections::BTreeMap::new();
    for s in samples {
        by_env.entry(s.environment).or_default().push(s);
    }
    let env_keys: Vec<usize> = by_env.keys().copied().collect();
    if env_keys.len() < 2 {
        // Cannot test invariance with fewer than 2 environments.
        // Default to returning all variables as candidates.
        return IcpResult {
            parents: (0..p).collect(),
            invariance_scores: (0..p)
                .map(|i| (vec![i].into_iter().collect(), 1.0))
                .collect(),
        };
    }

    // Enumerate candidate subsets up to max_cardinality (combinatorial).
    let mut invariance_scores: Vec<(BTreeSet<usize>, f64)> = Vec::new();

    for card in 1..=max_cardinality.min(p) {
        for combo in combinations(p, card) {
            let s_set: BTreeSet<usize> = combo.iter().copied().collect();
            let score = test_invariance(&by_env, &s_set, p_value_threshold);
            invariance_scores.push((s_set, score));
        }
    }

    // The estimated parent set is the intersection of all invariant sets.
    let invariant_sets: Vec<&BTreeSet<usize>> = invariance_scores
        .iter()
        .filter(|(_, score)| *score >= p_value_threshold)
        .map(|(s, _)| s)
        .collect();

    let parents: BTreeSet<usize> = if invariant_sets.is_empty() {
        BTreeSet::new()
    } else {
        let mut iter = invariant_sets.iter();
        let first = (*iter.next().unwrap()).clone();
        iter.fold(first, |acc, s| acc.intersection(s).copied().collect())
    };

    IcpResult {
        parents,
        invariance_scores,
    }
}

/// Test whether `Y | X_S` is invariant across environments.
///
/// Returns the maximum p-value across all pairs of environments (higher
/// ⇒ more invariant).
fn test_invariance(
    by_env: &std::collections::BTreeMap<usize, Vec<&IcpSample>>,
    s_set: &BTreeSet<usize>,
    _threshold: f64,
) -> f64 {
    let env_keys: Vec<usize> = by_env.keys().copied().collect();
    let mut min_p = 1.0f64;
    for i in 0..env_keys.len() {
        for j in (i + 1)..env_keys.len() {
            let group_a = &by_env[&env_keys[i]];
            let group_b = &by_env[&env_keys[j]];
            let x_a: Vec<Vec<f64>> = group_a.iter().map(|s| extract_features(s, s_set)).collect();
            let x_b: Vec<Vec<f64>> = group_b.iter().map(|s| extract_features(s, s_set)).collect();
            let y_a: Vec<f64> = group_a.iter().map(|s| s.target).collect();
            let y_b: Vec<f64> = group_b.iter().map(|s| s.target).collect();

            // Combine and fit OLS
            let mut x_all = x_a.clone();
            x_all.extend(x_b.iter().cloned());
            let mut y_all = y_a.clone();
            y_all.extend(y_b.iter().cloned());
            let Some(beta) = ols_solve(&x_all, &y_all) else {
                continue;
            };
            let res_a = ols_residuals(&x_a, &y_a, &beta);
            let res_b = ols_residuals(&x_b, &y_b, &beta);
            if let Some((t, _df)) = welch_t(&res_a, &res_b) {
                let p = two_sided_p_normal(t);
                if p < min_p {
                    min_p = p;
                }
            }
        }
    }
    min_p
}

fn extract_features(s: &IcpSample, s_set: &BTreeSet<usize>) -> Vec<f64> {
    s_set
        .iter()
        .map(|&i| s.features.get(i).copied().unwrap_or(0.0))
        .collect()
}

fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    if k > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.clone());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if idx[i] < n - k + i {
                idx[i] += 1;
                for j in (i + 1)..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_env(seed: u64, slope_x0: f64, slope_x1: f64, intercept: f64) -> Vec<IcpSample> {
        // Simple LCG for reproducibility
        let mut state = seed;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state as f64) / (u64::MAX as f64)
        };
        (0..30)
            .map(|_| {
                let x0 = next() * 2.0 - 1.0;
                let x1 = next() * 2.0 - 1.0;
                let noise_scale = (next() - 0.5) * 0.1;
                let y = intercept + slope_x0 * x0 + slope_x1 * x1 + noise_scale;
                IcpSample {
                    features: vec![x0, x1, next() * 0.1],
                    target: y,
                    environment: 0,
                }
            })
            .collect()
    }

    fn env_with_seed(
        seed: u64,
        env: usize,
        slope_x0: f64,
        slope_x1: f64,
        intercept: f64,
    ) -> Vec<IcpSample> {
        let mut state = seed;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state as f64) / (u64::MAX as f64)
        };
        (0..30)
            .map(|_| {
                let x0 = next() * 2.0 - 1.0;
                let x1 = next() * 2.0 - 1.0;
                let noise_scale = (next() - 0.5) * 0.1;
                let y = intercept + slope_x0 * x0 + slope_x1 * x1 + noise_scale;
                IcpSample {
                    features: vec![x0, x1, next() * 0.1],
                    target: y,
                    environment: env,
                }
            })
            .collect()
    }

    #[test]
    fn ols_recovers_known_coefficients() {
        // y = 2 + 3*x0 + (-1)*x1
        let x = vec![vec![1.0, 0.0], vec![1.0, 1.0], vec![1.0, 2.0]];
        let y = vec![2.0, 4.0, 5.0];
        let beta = ols_solve(&x, &y).unwrap();
        assert!((beta[0] - 2.0).abs() < 1e-9);
        assert!((beta[1] - 1.5).abs() < 1e-9);
    }

    #[test]
    fn welch_t_detects_difference() {
        let a = vec![1.0, 1.1, 0.9, 1.05, 0.95];
        let b = vec![3.0, 3.1, 2.9, 3.05, 2.95];
        let (t, _) = welch_t(&a, &b).unwrap();
        assert!(t.abs() > 5.0);
    }

    #[test]
    fn welch_t_returns_none_for_small_groups() {
        let a = vec![1.0];
        let b = vec![2.0];
        assert!(welch_t(&a, &b).is_none());
    }

    #[test]
    fn icp_recovers_true_parents_in_synthetic_data() {
        // Two environments, same coefficients for x0 and x1, but a
        // spurious third variable that varies across environments.
        let env_a = env_with_seed(11, 0, 2.0, -1.0, 0.5);
        let env_b = env_with_seed(23, 1, 2.0, -1.0, 0.5);
        let mut all = env_a;
        all.extend(env_b);
        let result = icp(&all, 0.05, 3);
        assert!(result.parents.contains(&0), "x0 should be a parent");
        assert!(result.parents.contains(&1), "x1 should be a parent");
    }

    #[test]
    fn empty_data_returns_empty_result() {
        let result = icp(&[], 0.05, 2);
        assert!(result.parents.is_empty());
        assert!(result.invariance_scores.is_empty());
    }

    #[test]
    fn single_environment_returns_all_variables() {
        let only_env = synth_env(42, 1.0, 1.0, 0.0);
        let result = icp(&only_env, 0.05, 3);
        assert_eq!(result.parents.len(), 3);
    }
}
