//! The timezone of the day, hour and weekday buckets (prompt index `day`, `hour`, `weekday`).
//!
//! Python read the process timezone (`TZ`, `time.tzname`); here it is a value handed to the
//! [`crate::indexer::Indexer`], so tests pin one without touching the environment, and the desktop
//! app can follow the OS setting. Its [`Zone::name`] is part of the rules version: changing the
//! zone rebuilds the index.

use std::fmt::Debug;

use jiff::Timestamp;
use jiff::tz::Offset;
use serde::{Deserialize, Serialize};

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
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }
}

/// The operating system's local timezone, with its daylight saving rules (`jiff::tz::TimeZone::system()`). What the
/// daemon and the desktop app use; never in tests. Its name includes the current offset rules
/// (e.g. the IANA name when the OS reports one), so moving the machine to another zone rebuilds
/// the index.
#[derive(Clone, Copy, Debug, Default)]
pub struct MachineZone;

impl Zone for MachineZone {
    fn offset_at(&self, _instant: Timestamp) -> Offset {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
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
pub fn local_time(_received_at: &str, _zone: &dyn Zone) -> gitprompt_core::Result<LocalTime> {
    todo!()
}
