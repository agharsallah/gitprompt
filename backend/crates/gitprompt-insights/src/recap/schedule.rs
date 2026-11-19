//! When the weekly recap is due: the local weekday and hour of the preferences, in the zone's
//! offset at that instant, so a daylight saving change moves the instant, never the local hour.

use jiff::civil::{Date, DateTime};
use jiff::{SignedDuration, Timestamp, ToSpan};

use gitprompt_core::preferences::Preferences;

use crate::zone::Zone;

/// The instant of `local` in `zone`. A local time that happens twice (the hour a clock goes back)
/// is its first instant; one that never happens (the hour a clock goes forward) is read with the
/// offset before the change, so it lands just after the gap.
fn instant_of(local: DateTime, zone: &dyn Zone) -> Timestamp {
    let naive = local.to_zoned(jiff::tz::TimeZone::UTC).map_or(Timestamp::UNIX_EPOCH, |z| z.timestamp());
    let around = [naive - SignedDuration::from_hours(24), naive + SignedDuration::from_hours(24)];
    let candidates: Vec<(Timestamp, bool)> = around
        .iter()
        .map(|at| {
            let offset = zone.offset_at(*at);
            let instant = naive - SignedDuration::from_secs(i64::from(offset.seconds()));
            (instant, zone.offset_at(instant) == offset)
        })
        .collect();
    let valid = candidates.iter().filter(|(_, valid)| *valid).map(|(at, _)| *at).min();
    valid.unwrap_or_else(|| candidates.iter().map(|(at, _)| *at).max().unwrap_or(naive))
}

/// The local calendar date of `instant` in `zone`.
fn local_date(instant: Timestamp, zone: &dyn Zone) -> Date {
    zone.offset_at(instant).to_datetime(instant).date()
}

/// The recap instant on `date` (its local hour from the preferences).
fn on(date: Date, prefs: &Preferences) -> DateTime {
    date.at(i8::try_from(prefs.recap_hour.min(23)).unwrap_or(0), 0, 0, 0)
}

/// Days from `date` forward to the recap weekday (0 when `date` is one).
fn days_until_weekday(date: Date, prefs: &Preferences) -> i8 {
    (prefs.recap_weekday.number() - date.weekday().to_monday_one_offset()).rem_euclid(7)
}

/// `date` moved by `days`, kept as is at the edge of the calendar.
fn shift(date: Date, days: i32) -> Date {
    date.checked_add(days.days()).unwrap_or(date)
}

/// The first recap instant strictly after `now`.
pub fn next_recap(now: Timestamp, prefs: &Preferences, zone: &dyn Zone) -> Timestamp {
    let today = local_date(now, zone);
    let day = shift(today, i32::from(days_until_weekday(today, prefs)));
    let at = instant_of(on(day, prefs), zone);
    if at > now { at } else { instant_of(on(shift(day, 7), prefs), zone) }
}

/// The latest recap instant at or before `now`: the week the daemon owes a recap for.
pub fn last_recap(now: Timestamp, prefs: &Preferences, zone: &dyn Zone) -> Timestamp {
    let today = local_date(now, zone);
    let day = shift(today, i32::from(days_until_weekday(today, prefs)) - 7);
    let next_week = shift(day, 7);
    let at = instant_of(on(next_week, prefs), zone);
    if at <= now { at } else { instant_of(on(day, prefs), zone) }
}

/// The start of the week a recap due at `due` sums up: the same local time seven days before.
pub fn week_before(due: Timestamp, zone: &dyn Zone) -> Timestamp {
    let local = zone.offset_at(due).to_datetime(due);
    let earlier = local.checked_sub(7.days()).unwrap_or(local);
    instant_of(earlier, zone)
}
