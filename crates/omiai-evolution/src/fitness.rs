//! Fitness evaluation helpers for genetic algorithms and genetic programming.

/// Higher is better by convention across this crate.
pub type Fitness = f64;

/// Mean squared error → fitness (higher better): `1 / (1 + mse)`.
pub fn mse_to_fitness(predictions: &[f64], targets: &[f64]) -> Fitness {
    let n = predictions.len().min(targets.len());
    if n == 0 {
        return 0.0;
    }
    let mse: f64 = predictions
        .iter()
        .zip(targets.iter())
        .map(|(p, t)| (p - t).powi(2))
        .sum::<f64>()
        / n as f64;
    1.0 / (1.0 + mse)
}

/// Absolute error fitness.
pub fn mae_to_fitness(predictions: &[f64], targets: &[f64]) -> Fitness {
    let n = predictions.len().min(targets.len());
    if n == 0 {
        return 0.0;
    }
    let mae: f64 = predictions
        .iter()
        .zip(targets.iter())
        .map(|(p, t)| (p - t).abs())
        .sum::<f64>()
        / n as f64;
    1.0 / (1.0 + mae)
}

/// Rank individuals by fitness descending; returns indices.
pub fn rank_indices(fitnesses: &[Fitness]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..fitnesses.len()).collect();
    idx.sort_by(|&a, &b| {
        fitnesses[b]
            .partial_cmp(&fitnesses[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_prediction_high_fitness() {
        let p = vec![1.0, 2.0, 3.0];
        assert!((mse_to_fitness(&p, &p) - 1.0).abs() < 1e-9);
    }
}
