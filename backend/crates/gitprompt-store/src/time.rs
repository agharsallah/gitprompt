//! Times as the store writes them: UTC, ISO-8601 with microseconds and `+00:00` (Python's
//! `datetime.isoformat()`, with the microseconds always written). Such strings sort as times.

use jiff::{SignedDuration, Timestamp};

pub(crate) fn iso(t: Timestamp) -> String {
    t.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
}

/// `t` minus `by`; none when that is out of the range jiff represents.
pub(crate) fn before(t: Timestamp, by: SignedDuration) -> Option<Timestamp> {
    t.checked_sub(by).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_has_microseconds_and_a_utc_offset() {
        let t: Timestamp = "2026-11-03T10:00:00.1234567Z".parse().unwrap();
        assert_eq!(iso(t), "2026-11-03T10:00:00.123456+00:00");
        assert_eq!(iso("2026-11-03T10:00:00Z".parse().unwrap()), "2026-11-03T10:00:00.000000+00:00");
    }
}
