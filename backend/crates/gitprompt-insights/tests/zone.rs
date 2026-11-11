//! Day, hour and weekday buckets in an injected timezone (zone.rs). Python read the process `TZ`;
//! these tests never touch the environment.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use jiff::civil::date;
use jiff::tz::{Offset, TimeZone};
use pretty_assertions::assert_eq;

use common::{Berlin2026, offset_hours, utc};
use gitprompt_core::Error;
use gitprompt_insights::zone::{LocalTime, Zone, local_time};

fn local(day: &str, hour: u8, weekday: u8) -> LocalTime {
    LocalTime { day: day.into(), hour, weekday }
}

#[test]
fn a_utc_instant_late_in_the_day_falls_on_the_next_local_day_east_of_utc() {
    assert_eq!(local_time("2026-11-03T23:30:00+00:00", &offset_hours(1)).unwrap(), local("2026-11-04", 0, 3));
}

#[test]
fn an_early_utc_instant_falls_on_the_previous_local_day_west_of_utc() {
    let new_york_winter = Offset::from_seconds(-5 * 3600).unwrap();
    assert_eq!(local_time("2026-11-03T02:00:00+00:00", &new_york_winter).unwrap(), local("2026-11-02", 21, 1));
}

#[test]
fn a_timestamp_with_another_offset_is_read_in_that_offset() {
    assert_eq!(local_time("2026-11-03T10:00:00+05:00", &utc()).unwrap(), local("2026-11-03", 5, 2));
}

#[test]
fn z_timestamps_with_fractional_seconds_are_read() {
    assert_eq!(local_time("2026-11-07T10:00:20.000Z", &utc()).unwrap(), local("2026-11-07", 10, 6));
}

#[test]
fn microsecond_timestamps_as_the_store_writes_them_are_read() {
    assert_eq!(local_time("2026-11-07T10:00:20.123456+00:00", &utc()).unwrap(), local("2026-11-07", 10, 6));
}

#[test]
fn daylight_saving_rules_apply_at_the_instant_of_the_prompt() {
    assert_eq!(local_time("2026-11-03T10:00:00+00:00", &Berlin2026).unwrap(), local("2026-11-03", 11, 2));
    assert_eq!(local_time("2026-07-01T22:30:00+00:00", &Berlin2026).unwrap(), local("2026-07-02", 0, 4));
}

#[test]
fn a_value_that_is_not_a_timestamp_is_an_invalid_input_naming_it() {
    let err = local_time("yesterday", &utc()).unwrap_err();
    assert!(matches!(&err, Error::Invalid(message) if message.contains("yesterday")), "{err:?}");
}

#[test]
fn a_fixed_offset_is_named_by_its_offset() {
    assert_eq!(utc().name(), "+00:00");
    assert_eq!(offset_hours(1).name(), "+01:00");
    assert_eq!(Offset::from_seconds(-(5 * 3600 + 1800)).unwrap().name(), "-05:30");
}

#[test]
fn a_fixed_offset_has_the_same_offset_at_every_instant() {
    let summer = date(2026, 7, 1).at(12, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp();
    let winter = date(2026, 1, 1).at(12, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp();
    assert_eq!((offset_hours(1).offset_at(summer), offset_hours(1).offset_at(winter)), (offset_hours(1), offset_hours(1)));
}
