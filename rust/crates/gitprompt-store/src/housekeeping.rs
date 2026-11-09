//! Storage hygiene: apply today's capture rules to what was captured earlier, and retention.
//!
//! * noise    — stored events the current specs would no longer keep
//! * expired  — raw events older than N days (prompts and annotations are never removed)
//! * selected — raw events of one source and/or received before a time
//!
//! `compact` plans by default and changes nothing; with `apply` it deletes, marks the index stale,
//! then VACUUMs.

use std::collections::BTreeMap;
use std::sync::Arc;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use gitprompt_core::Result;
use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::spec::Spec;

use crate::Store;

/// How often the daemon applies `event_retention_days`.
pub const RETENTION_EVERY: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    #[serde(rename = "dropped by spec")]
    DroppedBySpec,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "selected")]
    Selected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removal {
    pub source: String,
    pub event: Option<String>,
    pub rows: u64,
    pub bytes: u64,
    pub why: Why,
    /// Only rows received before this time.
    pub before: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub removals: Vec<Removal>,
    pub size_before: u64,
    /// Set once applied.
    pub size_after: Option<u64>,
}

impl Report {
    pub fn rows(&self) -> u64 {
        todo!()
    }
    pub fn bytes(&self) -> u64 {
        todo!()
    }
}

/// The received_at before which an event is older than `days` days. `days < 1` = `Error::Invalid`.
pub fn cutoff(_days: i64, _now: Timestamp) -> Result<String> {
    todo!()
}

/// Stored events their source's spec no longer keeps. Events of a source without a spec are kept.
pub fn noise(_store: &Store, _specs: &BTreeMap<String, Arc<Spec>>) -> Result<Vec<Removal>> {
    todo!()
}

/// Raw events older than `days` days, except the (source, event) kinds in `skip`.
pub fn expired(_store: &Store, _days: i64, _now: Timestamp, _skip: &[Removal]) -> Result<Vec<Removal>> {
    todo!()
}

/// Raw events of `source` (`None` = every source) received before `before` (`None` = any time).
pub fn selected(_store: &Store, _source: Option<&str>, _before: Option<&str>) -> Result<Vec<Removal>> {
    todo!()
}

/// Delete the events in one transaction; when anything was removed the index is invalidated.
pub fn remove(_store: &Store, _removals: &[Removal]) -> Result<u64> {
    todo!()
}

/// Report what `removals` would delete; with `apply`, delete it and VACUUM.
pub fn plan(_store: &Store, _removals: Vec<Removal>, _apply: bool) -> Result<Report> {
    todo!()
}

/// noise (+ expired when `older_than_days`), planned or applied.
pub fn compact(
    _store: &Store,
    _specs: &BTreeMap<String, Arc<Spec>>,
    _older_than_days: Option<i64>,
    _now: Timestamp,
    _apply: bool,
) -> Result<Report> {
    todo!()
}

/// One retention pass with the `event_retention_days` preference (re-read every pass). Returns the
/// number of events deleted; 0 when retention is off.
pub fn retention_pass(_store: &Store, _preferences: &PreferenceFile, _now: Timestamp) -> Result<u64> {
    todo!()
}
