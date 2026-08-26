//! Selection strategies: tournament, roulette wheel, and lexicase.

use rand::Rng;

use super::fitness::Fitness;

/// Selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    Tournament { k: usize },
    Roulette,
    Lexicase,
}

/// Select one individual index given fitness values.
pub fn select(fitnesses: &[Fitness], strategy: SelectionStrategy, rng: &mut impl Rng) -> usize {
    match strategy {
        SelectionStrategy::Tournament { k } => tournament(fitnesses, k.max(1), rng),
        SelectionStrategy::Roulette => roulette(fitnesses, rng),
        SelectionStrategy::Lexicase => {
            // Without multi-objective cases, fall back to tournament
            tournament(fitnesses, 3, rng)
        }
    }
}

/// Tournament selection of size `k`.
pub fn tournament(fitnesses: &[Fitness], k: usize, rng: &mut impl Rng) -> usize {
    let n = fitnesses.len();
    if n == 0 {
        return 0;
    }
    let mut best = rng.gen_range(0..n);
    for _ in 1..k {
        let challenger = rng.gen_range(0..n);
        if fitnesses[challenger] > fitnesses[best] {
            best = challenger;
        }
    }
    best
}

/// Fitness-proportionate (roulette) selection.
pub fn roulette(fitnesses: &[Fitness], rng: &mut impl Rng) -> usize {
    let n = fitnesses.len();
    if n == 0 {
        return 0;
    }
    // Shift to non-negative
    let min = fitnesses.iter().cloned().fold(f64::INFINITY, f64::min);
    let shifted: Vec<f64> = fitnesses
        .iter()
        .map(|f| (f - min + 1e-9).max(0.0))
        .collect();
    let total: f64 = shifted.iter().sum();
    if total <= 0.0 {
        return rng.gen_range(0..n);
    }
    let mut r = rng.r#gen::<f64>() * total;
    for (i, w) in shifted.iter().enumerate() {
        r -= w;
        if r <= 0.0 {
            return i;
        }
    }
    n - 1
}

/// Lexicase selection over a matrix of case fitnesses (rows = individuals,
/// cols = test cases). Filters the population case-by-case.
pub fn lexicase(case_fitness: &[Vec<Fitness>], rng: &mut impl Rng) -> usize {
    let n = case_fitness.len();
    if n == 0 {
        return 0;
    }
    let n_cases = case_fitness[0].len();
    if n_cases == 0 {
        return rng.gen_range(0..n);
    }
    let mut candidates: Vec<usize> = (0..n).collect();
    let mut case_order: Vec<usize> = (0..n_cases).collect();
    // Shuffle cases
    for i in (1..case_order.len()).rev() {
        let j = rng.gen_range(0..=i);
        case_order.swap(i, j);
    }
    for &c in &case_order {
        if candidates.len() <= 1 {
            break;
        }
        let best = candidates
            .iter()
            .map(|&i| case_fitness[i][c])
            .fold(f64::NEG_INFINITY, f64::max);
        candidates.retain(|&i| (case_fitness[i][c] - best).abs() < 1e-12);
    }
    candidates[rng.gen_range(0..candidates.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn tournament_prefers_fitter() {
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let fit = vec![0.1, 0.9, 0.2];
        let mut counts = [0usize; 3];
        for _ in 0..200 {
            counts[tournament(&fit, 3, &mut rng)] += 1;
        }
        assert!(counts[1] > counts[0]);
    }
}
