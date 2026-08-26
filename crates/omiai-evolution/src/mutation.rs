//! Point mutation operators for real-valued genomes and discrete genes.

use rand::Rng;

/// Gaussian mutation: each gene mutated with probability `rate` by N(0, sigma).
pub fn mutate_real(genome: &mut [f64], rate: f64, sigma: f64, rng: &mut impl Rng) {
    for g in genome.iter_mut() {
        if rng.r#gen::<f64>() < rate {
            let noise: f64 = rng.gen_range(-1.0..1.0) * sigma;
            *g += noise;
        }
    }
}

/// Integer gene mutation within `[lo, hi]`.
pub fn mutate_int(genome: &mut [usize], rate: f64, lo: usize, hi: usize, rng: &mut impl Rng) {
    if hi < lo {
        return;
    }
    for g in genome.iter_mut() {
        if rng.r#gen::<f64>() < rate {
            *g = rng.gen_range(lo..=hi);
        }
    }
}

/// Bit-flip mutation for binary genomes.
pub fn mutate_bits(genome: &mut [bool], rate: f64, rng: &mut impl Rng) {
    for g in genome.iter_mut() {
        if rng.r#gen::<f64>() < rate {
            *g = !*g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn mutation_changes_something_at_high_rate() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut g = vec![0.0; 20];
        mutate_real(&mut g, 1.0, 1.0, &mut rng);
        assert!(g.iter().any(|&x| x != 0.0));
    }
}
