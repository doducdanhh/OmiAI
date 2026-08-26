//! Crossover operators: single-point, two-point, and uniform.

use rand::Rng;

/// Single-point crossover for real genomes.
pub fn single_point(
    parent_a: &[f64],
    parent_b: &[f64],
    rng: &mut impl Rng,
) -> (Vec<f64>, Vec<f64>) {
    let n = parent_a.len().min(parent_b.len());
    if n == 0 {
        return (vec![], vec![]);
    }
    let point = rng.gen_range(0..n);
    let mut c1 = parent_a[..n].to_vec();
    let mut c2 = parent_b[..n].to_vec();
    for i in point..n {
        c1[i] = parent_b[i];
        c2[i] = parent_a[i];
    }
    (c1, c2)
}

/// Uniform crossover: each gene independently from either parent.
pub fn uniform(parent_a: &[f64], parent_b: &[f64], rng: &mut impl Rng) -> (Vec<f64>, Vec<f64>) {
    let n = parent_a.len().min(parent_b.len());
    let mut c1 = Vec::with_capacity(n);
    let mut c2 = Vec::with_capacity(n);
    for i in 0..n {
        if rng.r#gen::<bool>() {
            c1.push(parent_a[i]);
            c2.push(parent_b[i]);
        } else {
            c1.push(parent_b[i]);
            c2.push(parent_a[i]);
        }
    }
    (c1, c2)
}

/// Single-point crossover for discrete genomes.
pub fn single_point_usize(
    parent_a: &[usize],
    parent_b: &[usize],
    rng: &mut impl Rng,
) -> (Vec<usize>, Vec<usize>) {
    let n = parent_a.len().min(parent_b.len());
    if n == 0 {
        return (vec![], vec![]);
    }
    let point = rng.gen_range(0..n);
    let mut c1 = parent_a[..n].to_vec();
    let mut c2 = parent_b[..n].to_vec();
    for i in point..n {
        c1[i] = parent_b[i];
        c2[i] = parent_a[i];
    }
    (c1, c2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn single_point_preserves_length() {
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let (c1, c2) = single_point(&a, &b, &mut rng);
        assert_eq!(c1.len(), 4);
        assert_eq!(c2.len(), 4);
    }
}
