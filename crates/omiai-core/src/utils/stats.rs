//! Statistics helpers shared across probabilistic and causal modules.

/// Arithmetic mean.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Unbiased sample variance (Bessel's correction).
pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64
}

/// Sample standard deviation.
pub fn std_dev(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// Pearson correlation coefficient.
pub fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }
    let mx = mean(&x[..n]);
    let my = mean(&y[..n]);
    let mut num = 0.0;
    let mut dx = 0.0;
    let mut dy = 0.0;
    for i in 0..n {
        let a = x[i] - mx;
        let b = y[i] - my;
        num += a * b;
        dx += a * a;
        dy += b * b;
    }
    let denom = (dx * dy).sqrt();
    if denom < 1e-15 { 0.0 } else { num / denom }
}

/// Softmin / log-sum-exp for numerical stability.
pub fn log_sum_exp(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return f64::NEG_INFINITY;
    }
    let m = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !m.is_finite() {
        return m;
    }
    let sum: f64 = xs.iter().map(|x| (x - m).exp()).sum();
    m + sum.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_variance_basic() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((mean(&v) - 2.0).abs() < 1e-9);
        assert!((variance(&v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pearson_perfect() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![2.0, 4.0, 6.0];
        assert!((pearson(&x, &y) - 1.0).abs() < 1e-9);
    }
}
