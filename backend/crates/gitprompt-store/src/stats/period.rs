//! The period before a filter's own: the same length of time, ending where the filter's period
//! starts. The aggregate over it is the same query with only `since` and `until` moved, so the two
//! periods compare figure by figure.
//!
//! Lengths are in absolute time, not in calendar days: the dashboard's ranges are rolling windows
//! (`now − 7 × 24 h`), so the period before is equally long whatever the time zone or a daylight
//! saving change in between, and it ends exactly where the current one starts (no gap, no overlap).

use jiff::Timestamp;

use crate::filter::PromptFilter;
use crate::time::{before, iso};

/// The span of a period, UTC instants in the store's format (`since` inclusive, `until` exclusive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period {
    pub since: String,
    pub until: String,
}

impl PromptFilter {
    /// The same filter over the period of equal length just before its own, with that period.
    /// None without a start (all time has no "before"), with a start that is not a time, or with
    /// a period that is empty or ends before it starts. Without `until`, the period ends at `now`.
    pub fn previous_period(&self, now: Timestamp) -> Option<(PromptFilter, Period)> {
        let since = instant(self.since.as_deref()?)?;
        let until = match self.until.as_deref() {
            Some(text) => instant(text)?,
            None => now,
        };
        let length = until.duration_since(since);
        if !length.is_positive() {
            return None;
        }
        let period = Period { since: iso(before(since, length)?), until: iso(since) };
        let filter = PromptFilter { since: Some(period.since.clone()), until: Some(period.until.clone()), before: None, ..self.clone() };
        Some((filter, period))
    }
}

fn instant(text: &str) -> Option<Timestamp> {
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(since: Option<&str>, until: Option<&str>) -> PromptFilter {
        PromptFilter { since: since.map(String::from), until: until.map(String::from), ..PromptFilter::default() }
    }

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    #[test]
    fn the_period_before_ends_where_the_filter_starts() {
        let (previous, period) = filter(Some("2026-11-11T12:00:00+00:00"), None).previous_period(at("2026-11-18T12:00:00Z")).unwrap();
        assert_eq!(period.since, "2026-11-04T12:00:00.000000+00:00");
        assert_eq!(period.until, "2026-11-11T12:00:00.000000+00:00");
        assert_eq!(previous.since.as_deref(), Some(period.since.as_str()));
        assert_eq!(previous.until.as_deref(), Some(period.until.as_str()));
    }

    #[test]
    fn an_until_sets_the_length_and_other_filters_are_kept() {
        let mut f = filter(Some("2026-11-10T00:00:00Z"), Some("2026-11-11T00:00:00Z"));
        f.repo = Some("gitprompt".into());
        f.before = Some(42);
        let (previous, period) = f.previous_period(at("2030-01-01T00:00:00Z")).unwrap();
        assert_eq!(period.since, "2026-11-09T00:00:00.000000+00:00");
        assert_eq!(previous.repo.as_deref(), Some("gitprompt"));
        assert_eq!(previous.before, None, "a paging cursor belongs to the current page");
    }

    #[test]
    fn no_start_or_an_empty_period_has_no_before() {
        let now = at("2026-11-18T12:00:00Z");
        assert_eq!(filter(None, None).previous_period(now), None);
        assert_eq!(filter(Some("not a time"), None).previous_period(now), None);
        assert_eq!(filter(Some("2026-11-18T12:00:00Z"), None).previous_period(now), None);
        assert_eq!(filter(Some("2026-11-19T00:00:00Z"), None).previous_period(now), None);
    }
}
