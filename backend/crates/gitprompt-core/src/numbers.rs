//! Number formatting shared by every crate, so a value rounds the same everywhere it is shown.

/// Python's `round(x, digits)`: the decimal closest to the exact binary value of `x`, ties to even.
///
/// Multiplying first (`(x * 10).round() / 10`) is wrong: `76.45` is stored as `76.4500000000000028…`,
/// which Python rounds to `76.5`, but `76.45 * 10` is exactly `764.5`, which ties to `76.4`. Rust's
/// fixed-precision formatting rounds the exact binary value, so the formatted text is parsed back.
pub fn round(x: f64, digits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.digits$}").parse().unwrap_or(x)
}

#[cfg(test)]
#[expect(clippy::float_cmp, reason = "rounding tests: the exact float is the expected value")]
mod tests {
    use super::round;

    #[test]
    fn rounds_the_exact_binary_value_like_python() {
        // Expected values from CPython's round().
        assert_eq!(round(76.45, 1), 76.5);
        assert_eq!(round(2.675, 2), 2.67);
        assert_eq!(round(0.125, 2), 0.12);
        assert_eq!(round(0.375, 2), 0.38);
        assert_eq!(round(-1.25, 1), -1.2);
        assert_eq!(round(1234.56789, 4), 1234.5679);
    }

    #[test]
    fn keeps_non_finite_values() {
        assert!(round(f64::NAN, 1).is_nan());
        assert_eq!(round(f64::INFINITY, 1), f64::INFINITY);
    }
}
