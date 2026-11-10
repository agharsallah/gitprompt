//! The timezone of the day, hour and weekday buckets (prompt index `day`, `hour`, `weekday`).
//!
//! Python read the process timezone (`TZ`, `time.tzname`); here it is a value handed to the
//! [`crate::indexer::Indexer`], so tests pin one without touching the environment, and the desktop
//! app can follow the OS setting. Its [`Zone::name`] is part of the rules version: changing the
//! zone rebuilds the index.

use std::fmt::Debug;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::{Offset, TimeZone};
use serde::{Deserialize, Serialize};

use crate::time::offset_text;

/// A timezone: an offset from UTC per instant, so daylight saving rules apply at that instant.
pub trait Zone: Debug + Send + Sync {
    /// The offset from UTC in force at `instant`.
    fn offset_at(&self, instant: Timestamp) -> Offset;

    /// A stable name for the zone's rules (e.g. `Europe/Berlin`, `+01:00`): part of the rules
    /// version, so two zones with different rules must have different names.
    fn name(&self) -> String;
}

/// A fixed offset: no daylight saving. Its name is the offset, e.g. `+01:00`, `-05:30`, `+00:00`
/// (always `±HH:MM`; `Offset`'s `Display` shortens whole hours to `+01`).
impl Zone for Offset {
    fn offset_at(&self, _instant: Timestamp) -> Offset {
        *self
    }

    fn name(&self) -> String {
        offset_text(*self)
    }
}

/// The operating system's local timezone, with its daylight saving rules (`jiff::tz::TimeZone::system()`). What the
/// daemon and the desktop app use; never in tests. Its name includes the current offset rules
/// (e.g. the IANA name when the OS reports one), so moving the machine to another zone rebuilds
/// the index.
#[derive(Clone, Copy, Debug, Default)]
pub struct MachineZone;

impl MachineZone {
    /// The OS timezone (jiff caches it and notices when it changes); UTC, with a warning, when the
    /// OS reports none jiff can read.
    fn system() -> TimeZone {
        TimeZone::try_system().unwrap_or_else(|error| {
            tracing::warn!(%error, "the system timezone could not be read; using UTC");
            TimeZone::UTC
        })
    }
}

impl Zone for MachineZone {
    fn offset_at(&self, instant: Timestamp) -> Offset {
        Self::system().to_offset(instant)
    }

    /// The IANA name when the OS reports one, else the zone's offsets in January and July of this
    /// year (which tell its daylight saving rules apart), e.g. `local(+01:00/+02:00)`.
    fn name(&self) -> String {
        let zone = Self::system();
        if let Some(name) = zone.iana_name() {
            return name.to_owned();
        }
        let now = Timestamp::now();
        let year = now.to_zoned(zone.clone()).year();
        let offsets: Vec<String> = [1, 7]
            .into_iter()
            .map(|month| {
                let instant = Date::new(year, month, 1).and_then(|day| day.to_zoned(zone.clone())).map_or(now, |z| z.timestamp());
                offset_text(zone.to_offset(instant))
            })
            .collect();
        format!("local({})", offsets.join("/"))
    }
}

/// A prompt's place in local time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTime {
    /// Local calendar day, `YYYY-MM-DD`.
    pub day: String,
    /// 0..=23
    pub hour: u8,
    /// ISO weekday, 1 = Monday .. 7 = Sunday.
    pub weekday: u8,
}

/// `received_at` (ISO-8601 with an offset or `Z`, optional fractional seconds) in `zone`, with the
/// zone's offset at that instant. `Error::Invalid` naming the value when it is not a timestamp.
pub fn local_time(received_at: &str, zone: &dyn Zone) -> gitprompt_core::Result<LocalTime> {
    let instant: Timestamp =
        received_at.parse().map_err(|_| gitprompt_core::Error::Invalid(format!("not a timestamp with an offset: {received_at:?}")))?;
    let local = zone.offset_at(instant).to_datetime(instant);
    Ok(LocalTime {
        day: local.date().strftime("%Y-%m-%d").to_string(),
        // Both in range by construction: an hour is 0..=23, an ISO weekday 1..=7.
        hour: local.hour().unsigned_abs(),
        weekday: local.weekday().to_monday_one_offset().unsigned_abs(),
    })
}
