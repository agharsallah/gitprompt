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

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::spec::Spec;
use gitprompt_core::{Error, Result};

use crate::Store;
use crate::store::EventUsage;
use crate::time;

/// How often the daemon applies `event_retention_days`.
pub const RETENTION_EVERY: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "RemovalReason")]
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
        self.removals.iter().map(|r| r.rows).sum()
    }
    pub fn bytes(&self) -> u64 {
        self.removals.iter().map(|r| r.bytes).sum()
    }
}

impl Removal {
    fn of(usage: EventUsage, why: Why, before: Option<&str>) -> Removal {
        Removal { source: usage.source, event: usage.event, rows: usage.rows, bytes: usage.bytes, why, before: before.map(str::to_owned) }
    }
}

/// The `received_at` before which an event is older than `days` days. `days < 1` = `Error::Invalid`.
pub fn cutoff(days: i64, now: Timestamp) -> Result<String> {
    if days < 1 {
        return Err(Error::Invalid("retention must be at least 1 day".into()));
    }
    let age = days.checked_mul(24 * 3600).map(SignedDuration::from_secs);
    let before =
        age.and_then(|age| time::before(now, age)).ok_or_else(|| Error::Invalid(format!("retention of {days} days is too long")))?;
    Ok(time::iso(before))
}

/// Stored events their source's spec no longer keeps. Events of a source without a spec are kept.
pub fn noise(store: &Store, specs: &BTreeMap<String, Arc<Spec>>) -> Result<Vec<Removal>> {
    Ok(store
        .event_usage(None)?
        .into_iter()
        .filter(|u| specs.get(&u.source).is_some_and(|spec| !spec.keeps(u.event.as_deref())))
        .map(|u| Removal::of(u, Why::DroppedBySpec, None))
        .collect())
}

/// Raw events older than `days` days, except the (source, event) kinds in `skip`.
pub fn expired(store: &Store, days: i64, now: Timestamp, skip: &[Removal]) -> Result<Vec<Removal>> {
    let before = cutoff(days, now)?;
    let skipped = |u: &EventUsage| skip.iter().any(|r| r.source == u.source && r.event == u.event);
    Ok(store.event_usage(Some(&before))?.into_iter().filter(|u| !skipped(u)).map(|u| Removal::of(u, Why::Expired, Some(&before))).collect())
}

/// Raw events of `source` (`None` = every source) received before `before` (`None` = any time).
pub fn selected(store: &Store, source: Option<&str>, before: Option<&str>) -> Result<Vec<Removal>> {
    Ok(store
        .event_usage(before)?
        .into_iter()
        .filter(|u| source.is_none_or(|s| s == u.source))
        .map(|u| Removal::of(u, Why::Selected, before))
        .collect())
}

/// Delete the events in one transaction; when anything was removed the index is invalidated
/// (turn values change, so the prompt index rebuilds on its next read).
pub fn remove(store: &Store, removals: &[Removal]) -> Result<u64> {
    store.transaction(|store| {
        let mut removed = 0;
        for r in removals {
            removed += store.delete_events(&r.source, r.event.as_deref(), r.before.as_deref())?;
        }
        if removed > 0 {
            store.index().invalidate()?;
        }
        Ok(removed)
    })
}

/// Report what `removals` would delete; with `apply`, delete it and VACUUM.
pub fn plan(store: &Store, removals: Vec<Removal>, apply: bool) -> Result<Report> {
    let mut report = Report { size_before: store.size()?, removals, size_after: None };
    if apply {
        remove(store, &report.removals)?;
        store.vacuum()?;
        report.size_after = Some(store.size()?);
    }
    Ok(report)
}

/// noise (+ expired when `older_than_days`), planned or applied.
pub fn compact(
    store: &Store,
    specs: &BTreeMap<String, Arc<Spec>>,
    older_than_days: Option<i64>,
    now: Timestamp,
    apply: bool,
) -> Result<Report> {
    let mut removals = noise(store, specs)?;
    if let Some(days) = older_than_days {
        let old = expired(store, days, now, &removals)?;
        removals.extend(old);
    }
    plan(store, removals, apply)
}

/// One retention pass with the `event_retention_days` preference (re-read every pass). Returns the
/// number of events deleted; 0 when retention is off.
pub fn retention_pass(store: &Store, preferences: &PreferenceFile, now: Timestamp) -> Result<u64> {
    match preferences.effective().event_retention_days {
        Some(days) if days > 0 => {
            let removed = remove(store, &expired(store, days.into(), now, &[])?)?;
            if removed > 0 {
                tracing::info!(removed, days, "retention: deleted events");
            }
            Ok(removed)
        }
        _ => Ok(0),
    }
}
