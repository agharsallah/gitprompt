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

/// `digits` (ASCII, no sign) with a comma every three digits from the right.
fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A whole number with thousands separators: `1,234`, `-5`.
pub fn thousands(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{}", group(&n.unsigned_abs().to_string()))
}

/// Python's `round` to a whole number: halves to even. The cast saturates, which no count reaches.
#[expect(clippy::cast_possible_truncation, reason = "saturates, which no count reaches")]
pub fn whole(n: f64) -> i64 {
    n.round_ties_even() as i64
}

/// `$1,234.57`; `<$0.01` for a cost under a cent but not zero; zero is `$0.00`.
pub fn money(usd: f64) -> String {
    if usd != 0.0 && usd < 0.01 {
        return "<$0.01".to_string();
    }
    let fixed = format!("{usd:.2}");
    let (whole, cents) = fixed.split_once('.').unwrap_or((&fixed, "00"));
    format!("${}.{cents}", group(whole))
}

/// A duration as read: `42 s`, `3 min` (whole minutes, rounded down), `2 h 5 min`, `12 h` (minutes
/// shown under 10 h only), `1 h` (no `0 min`).
pub fn duration(seconds: f64) -> String {
    let s = whole(seconds);
    if s < 60 {
        return format!("{s} s");
    }
    let minutes = s.div_euclid(60);
    if s < 3600 {
        return format!("{minutes} min");
    }
    let (h, m) = (minutes.div_euclid(60), minutes.rem_euclid(60));
    if m != 0 && h < 10 { format!("{h} h {m} min") } else { format!("{h} h") }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "rounding tests: the exact float is the expected value")]
mod tests {
    use super::{duration, money, round, thousands};

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

    #[test]
    fn money_durations_and_counts_read_as_text() {
        assert_eq!(money(1234.567), "$1,234.57");
        assert_eq!(money(0.004), "<$0.01");
        assert_eq!(money(0.0), "$0.00");
        assert_eq!(duration(42.4), "42 s");
        assert_eq!(duration(4320.0), "1 h 12 min");
        assert_eq!(duration(3600.0), "1 h");
        assert_eq!(duration(12.0 * 3600.0 + 300.0), "12 h");
        assert_eq!(thousands(-1_234_567), "-1,234,567");
    }
}
