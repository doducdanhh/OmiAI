//! Canonical Genetic Algorithm (Holland): real-valued or bit genomes,
//! selection → crossover → mutation → replacement.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::crossover;
use super::fitness::Fitness;
use super::mutation;
use super::selection::{self, SelectionStrategy};

/// Population of real-valued individuals.
#[derive(Debug, Clone)]
pub struct Population {
    pub genomes: Vec<Vec<f64>>,
    pub fitnesses: Vec<Fitness>,
}

impl Population {
    pub fn random(size: usize, genes: usize, lo: f64, hi: f64, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let genomes = (0..size)
            .map(|_| (0..genes).map(|_| rng.gen_range(lo..hi)).collect())
            .collect();
        Self {
            genomes,
            fitnesses: vec![0.0; size],
        }
    }

    pub fn best_index(&self) -> usize {
        self.fitnesses
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn best_fitness(&self) -> Fitness {
        self.fitnesses
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

/// Run one generation of a GA.
pub fn evolve_generation<F>(
    pop: &mut Population,
    fitness_fn: F,
    strategy: SelectionStrategy,
    crossover_rate: f64,
    mutation_rate: f64,
    mutation_sigma: f64,
    rng: &mut impl Rng,
) where
    F: Fn(&[f64]) -> Fitness,
{
    // Evaluate
    for (g, f) in pop.genomes.iter().zip(pop.fitnesses.iter_mut()) {
        *f = fitness_fn(g);
    }

    let n = pop.genomes.len();
    if n == 0 {
        return;
    }
    let gene_len = pop.genomes[0].len();
    let mut next = Vec::with_capacity(n);

    // Elitism: keep best
    let best = pop.best_index();
    next.push(pop.genomes[best].clone());

    while next.len() < n {
        let i = selection::select(&pop.fitnesses, strategy, rng);
        let j = selection::select(&pop.fitnesses, strategy, rng);
        let (mut c1, mut c2) = if rng.r#gen::<f64>() < crossover_rate {
            crossover::single_point(&pop.genomes[i], &pop.genomes[j], rng)
        } else {
            (pop.genomes[i].clone(), pop.genomes[j].clone())
        };
        if c1.len() != gene_len {
            c1.resize(gene_len, 0.0);
        }
        if c2.len() != gene_len {
            c2.resize(gene_len, 0.0);
        }
        mutation::mutate_real(&mut c1, mutation_rate, mutation_sigma, rng);
        mutation::mutate_real(&mut c2, mutation_rate, mutation_sigma, rng);
        next.push(c1);
        if next.len() < n {
            next.push(c2);
        }
    }
    pop.genomes = next;
    pop.fitnesses = vec![0.0; n];
}

/// Run a full GA for `generations` and return the best genome.
pub fn run_ga<F>(
    pop_size: usize,
    genes: usize,
    generations: usize,
    fitness_fn: F,
    seed: u64,
) -> (Vec<f64>, Fitness)
where
    F: Fn(&[f64]) -> Fitness,
{
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut pop = Population::random(pop_size, genes, -2.0, 2.0, seed);
    let strategy = SelectionStrategy::Tournament { k: 3 };
    for _ in 0..generations {
        evolve_generation(&mut pop, &fitness_fn, strategy, 0.8, 0.1, 0.2, &mut rng);
    }
    // Final eval
    for (g, f) in pop.genomes.iter().zip(pop.fitnesses.iter_mut()) {
        *f = fitness_fn(g);
    }
    let bi = pop.best_index();
    (pop.genomes[bi].clone(), pop.fitnesses[bi])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ga_optimizes_sphere() {
        // Minimize sum x_i^2 → fitness = 1/(1+sum sq)
        let (best, fit) = run_ga(
            40,
            3,
            30,
            |g| {
                let s: f64 = g.iter().map(|x| x * x).sum();
                1.0 / (1.0 + s)
            },
            123,
        );
        assert!(fit > 0.5, "fit={fit}, best={best:?}");
    }
}
