//! Perception stubs: convert raw sensory vectors into symbolic atoms
//! for the knowledge / logic layers (neuro-symbolic bridge).

/// Threshold a continuous feature vector into discrete predicate atoms.
pub fn vector_to_atoms(features: &[f64], names: &[&str], threshold: f64) -> Vec<String> {
    features
        .iter()
        .zip(names.iter())
        .filter(|(v, _)| **v >= threshold)
        .map(|(_, n)| (*n).to_string())
        .collect()
}

/// Softmax over logits (for action heads / attention).
pub fn softmax(logits: &[f64]) -> Vec<f64> {
    if logits.is_empty() {
        return vec![];
    }
    let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|x| (x - max).exp()).collect();
    let z: f64 = exps.iter().sum();
    exps.into_iter().map(|e| e / z.max(1e-15)).collect()
}

/// One-hot encode an index.
pub fn one_hot(index: usize, n: usize) -> Vec<f64> {
    let mut v = vec![0.0; n];
    if index < n {
        v[index] = 1.0;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoms_from_vector() {
        let atoms = vector_to_atoms(&[0.1, 0.9, 0.5], &["a", "b", "c"], 0.6);
        assert_eq!(atoms, vec!["b"]);
    }

    #[test]
    fn softmax_sums_to_one() {
        let s = softmax(&[1.0, 2.0, 3.0]);
        let sum: f64 = s.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }
}
