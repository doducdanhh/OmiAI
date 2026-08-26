//! Kolmogorov complexity estimation via compression proxies.
//!
//! True K(x) is uncomputable; we use Shannon-style entropy of an empirical
//! symbol distribution and a simple run-length encoding length as upper
//! bounds (Cilibrasi & Vitányi normalized compression distance family).

/// Shannon entropy of a byte string (bits per byte), a weak K proxy.
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let n = data.len() as f64;
    let mut h = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// Approximate Kolmogorov complexity upper bound:  
/// `K̂(x) ≈ |RLE(x)|` in bits (run-length encoding length).
pub fn estimate_complexity(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let rle = run_length_encode(data);
    // Each run: 1 byte value + 4 bytes count → 40 bits (coarse)
    rle.len() as f64 * 40.0
}

/// Normalized Compression Distance (NCD) between two byte strings.
///
/// `NCD(x,y) = (C(xy) - min(C(x),C(y))) / max(C(x),C(y))`
pub fn normalized_compression_distance(x: &[u8], y: &[u8]) -> f64 {
    let cx = estimate_complexity(x);
    let cy = estimate_complexity(y);
    let mut xy = x.to_vec();
    xy.extend_from_slice(y);
    let cxy = estimate_complexity(&xy);
    let numer = cxy - cx.min(cy);
    let denom = cx.max(cy).max(1e-9);
    (numer / denom).clamp(0.0, 1.0)
}

fn run_length_encode(data: &[u8]) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut cur = data[0];
    let mut count = 1u32;
    for &b in &data[1..] {
        if b == cur && count < u32::MAX {
            count += 1;
        } else {
            out.push((cur, count));
            cur = b;
            count = 1;
        }
    }
    out.push((cur, count));
    out
}

/// Compressibility ratio: C(x) / (8 |x|). Lower ⇒ more regular.
pub fn compressibility(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    estimate_complexity(data) / (8.0 * data.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_string_low_entropy() {
        let data = vec![0u8; 100];
        assert!(shannon_entropy(&data) < 0.1);
        let randomish: Vec<u8> = (0..100).map(|i| (i * 7 + 3) as u8).collect();
        assert!(shannon_entropy(&randomish) > shannon_entropy(&data));
    }

    #[test]
    fn ncd_identical_is_small() {
        let x = b"aaaaaaaaaa";
        let d = normalized_compression_distance(x, x);
        assert!(d < 0.3, "NCD={d}");
    }
}
