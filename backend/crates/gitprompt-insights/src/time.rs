//! ISO-8601 timestamps as the store and the tools write them (an offset or `Z`, optional fractional
//! seconds), kept with the offset they were written in, as Python's `datetime.fromisoformat` does.

use std::fmt::Write as _;

use jiff::fmt::temporal::{Pieces, PiecesOffset};
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Timestamp};

/// An instant and the offset its text was written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Instant {
    pub at: Timestamp,
    pub offset: Offset,
}

impl Instant {
    /// `None` when `text` is not a timestamp with an offset.
    pub(crate) fn parse(text: &str) -> Option<Instant> {
        let at: Timestamp = text.parse().ok()?;
        let offset = match Pieces::parse(text).ok()?.offset()? {
            PiecesOffset::Numeric(numeric) => numeric.offset(),
            // `Z`, and any form jiff may add later: an instant written in UTC.
            _ => Offset::UTC,
        };
        Some(Instant { at, offset })
    }

    /// `seconds` later (to the microsecond, like a Python `timedelta`), in the same offset. An
    /// instant beyond the representable range stays where it is.
    pub(crate) fn plus_seconds(self, seconds: f64) -> Instant {
        // Saturating float-to-int: absurd durations (out of i64 microseconds) clamp, then fail the
        // checked add below and leave the instant unchanged.
        #[expect(clippy::cast_possible_truncation, reason = "saturating on purpose; the checked add rejects the clamped value")]
        let micros = (seconds * 1e6).round_ties_even() as i64;
        let at = self.at.checked_add(SignedDuration::from_micros(micros)).unwrap_or(self.at);
        Instant { at, ..self }
    }

    /// Python's `datetime.isoformat()`: `YYYY-MM-DDTHH:MM:SS`, `.ffffff` when there are microseconds,
    /// then the offset as `±HH:MM` (`±HH:MM:SS` when it has seconds).
    pub(crate) fn isoformat(self) -> String {
        let local = self.at.to_zoned(TimeZone::fixed(self.offset));
        let mut out = local.strftime("%Y-%m-%dT%H:%M:%S").to_string();
        let micros = local.subsec_nanosecond() / 1000;
        if micros != 0 {
            let _ = write!(out, ".{micros:06}");
        }
        out.push_str(&offset_text(self.offset));
        out
    }
}

/// `±HH:MM`, or `±HH:MM:SS` for an offset with seconds.
pub(crate) fn offset_text(offset: Offset) -> String {
    let total = offset.seconds();
    let sign = if total < 0 { '-' } else { '+' };
    let total = total.unsigned_abs();
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if seconds == 0 { format!("{sign}{hours:02}:{minutes:02}") } else { format!("{sign}{hours:02}:{minutes:02}:{seconds:02}") }
}

/// Seconds from `start` to `end`, rounded to a tenth; `None` when either can't be parsed.
pub(crate) fn seconds_between(start: &str, end: &str) -> Option<f64> {
    let (start, end) = (Instant::parse(start)?, Instant::parse(end)?);
    Some(crate::text::round(end.at.duration_since(start.at).as_secs_f64(), 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isoformat_keeps_the_offset_and_shows_microseconds_only_when_there_are_some() {
        let t = Instant::parse("2026-03-02T09:00:00.250Z").unwrap();
        assert_eq!(t.isoformat(), "2026-03-02T09:00:00.250000+00:00");
        assert_eq!(Instant::parse("2026-03-02T10:00:00+01:00").unwrap().plus_seconds(60.0).isoformat(), "2026-03-02T10:01:00+01:00");
    }

    #[test]
    fn text_that_is_not_a_timestamp_with_an_offset_is_none() {
        assert_eq!((Instant::parse("yesterday"), Instant::parse("")), (None, None));
    }
}
