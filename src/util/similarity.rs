/// Cosine similarity between two f32 embedding vectors, computed in f64 precision.
///
/// Returns 0.0 for empty, mismatched-length, or zero-norm inputs.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (f64::from(*x), f64::from(*y));
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

#[cfg(test)]
mod tests {
    use super::cosine_similarity;

    #[test]
    fn identical_vectors() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn empty_returns_zero() {
        assert!(cosine_similarity(&[], &[]).abs() < f64::EPSILON);
    }

    #[test]
    fn mismatched_length_returns_zero() {
        assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_norm_returns_zero() {
        assert!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]).abs() < f64::EPSILON);
    }
}
