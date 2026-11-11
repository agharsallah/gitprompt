//! Small statistics shared by the health monitor and the load benchmark.

/// Nearest-rank percentile `q` (0..=1) of values sorted ascending; the default value when empty.
pub fn percentile<T: Copy + Default>(sorted: &[T], q: f64) -> T {
    if sorted.is_empty() {
        return T::default();
    }
    #[expect(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "an index into a short list")]
    let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted.get(rank - 1).copied().unwrap_or_default()
}

/// The larger of two values that may be missing (a missing one never wins).
pub fn max_present<T: PartialOrd>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b > a { b } else { a }),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let values: Vec<u32> = (1..=100).collect();
        assert_eq!((percentile(&values, 0.5), percentile(&values, 0.95), percentile(&values, 1.0)), (50, 95, 100));
        assert_eq!(percentile::<u32>(&[], 0.95), 0);
        assert_eq!(percentile(&[7], 0.0), 7);
    }

    #[test]
    fn a_missing_value_never_wins() {
        assert_eq!(max_present(Some(1.5), None), Some(1.5));
        assert_eq!(max_present(None, Some(2)), Some(2));
        assert_eq!(max_present(Some(3), Some(9)), Some(9));
        assert_eq!(max_present::<u8>(None, None), None);
    }
}
