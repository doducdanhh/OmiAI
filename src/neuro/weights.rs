//! Sparse random fixed weight matrices for reservoir / liquid-state
//! machines. Weights are drawn once (seeded ChaCha RNG) and never trained
//! via backpropagation — only the readout adapts.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Generate a dense `rows × cols` matrix with i.i.d. entries ~ Uniform(-scale, scale).
pub fn random_matrix(rows: usize, cols: usize, scale: f64, seed: u64) -> Vec<Vec<f64>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..rows)
        .map(|_| (0..cols).map(|_| rng.gen_range(-scale..scale)).collect())
        .collect()
}

/// Sparse random matrix: each entry is non-zero with probability `density`,
/// then drawn from Uniform(-scale, scale). Zeros elsewhere.
pub fn sparse_random_matrix(
    rows: usize,
    cols: usize,
    density: f64,
    scale: f64,
    seed: u64,
) -> Vec<Vec<f64>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let density = density.clamp(0.0, 1.0);
    (0..rows)
        .map(|_| {
            (0..cols)
                .map(|_| {
                    if rng.r#gen::<f64>() < density {
                        rng.gen_range(-scale..scale)
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

/// Estimate spectral radius of a square matrix via power iteration.
pub fn spectral_radius(matrix: &[Vec<f64>], iters: usize) -> f64 {
    let n = matrix.len();
    if n == 0 {
        return 0.0;
    }
    let mut v = vec![1.0 / (n as f64).sqrt(); n];
    let mut lambda = 0.0;
    for _ in 0..iters {
        let mut w = vec![0.0; n];
        for i in 0..n {
            let row = &matrix[i];
            let mut s = 0.0;
            for j in 0..n.min(row.len()) {
                s += row[j] * v[j];
            }
            w[i] = s;
        }
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-15);
        for i in 0..n {
            v[i] = w[i] / norm;
        }
        // Rayleigh quotient
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..n {
            let row = &matrix[i];
            let mut av = 0.0;
            for j in 0..n.min(row.len()) {
                av += row[j] * v[j];
            }
            num += v[i] * av;
            den += v[i] * v[i];
        }
        lambda = num / den.max(1e-15);
    }
    lambda.abs()
}

/// Rescale a square matrix so its spectral radius equals `target`.
pub fn normalize_spectral_radius(matrix: &mut [Vec<f64>], target: f64) {
    let rho = spectral_radius(matrix, 50);
    if rho < 1e-12 {
        return;
    }
    let factor = target / rho;
    for row in matrix.iter_mut() {
        for x in row.iter_mut() {
            *x *= factor;
        }
    }
}

/// Matrix-vector product `W * x`.
pub fn matvec(matrix: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| row.iter().zip(x.iter()).map(|(w, xi)| w * xi).sum())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectral_normalize() {
        let mut w = random_matrix(8, 8, 1.0, 42);
        normalize_spectral_radius(&mut w, 0.9);
        let rho = spectral_radius(&w, 100);
        assert!((rho - 0.9).abs() < 0.15, "rho={rho}");
    }

    #[test]
    fn sparse_has_zeros() {
        let m = sparse_random_matrix(10, 10, 0.1, 1.0, 1);
        let zeros = m.iter().flatten().filter(|&&x| x == 0.0).count();
        assert!(zeros > 50);
    }
}
