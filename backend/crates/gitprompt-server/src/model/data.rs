//! The Data page: where the data lives, its size, backups, and the compact/delete housekeeping.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_store::housekeeping::Why;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Backup {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SourceEvents {
    pub source: String,
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "DataInfo")]
pub struct DataOut {
    pub home: String,
    pub db_path: String,
    pub db_bytes: u64,
    pub receiver: String,
    /// Stored raw events per source, largest first.
    pub events: Vec<SourceEvents>,
    /// Config-file copies taken before each connect/disconnect, newest first.
    pub backups: Vec<Backup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Removal")]
pub struct RemovalOut {
    pub source: String,
    pub event: Option<String>,
    pub rows: u64,
    pub bytes: u64,
    pub why: Why,
    pub before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CleanupReport")]
pub struct ReportOut {
    pub removals: Vec<RemovalOut>,
    pub rows: u64,
    pub bytes: u64,
    pub size_before: u64,
    pub size_after: Option<u64>,
    pub applied: bool,
}

/// Body of `POST /api/data/compact`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct CompactIn {
    /// Also remove raw events older than this (≥ 1, else 400).
    pub older_than_days: Option<i64>,
}

/// Body of `POST /api/data/delete-events`. At least one of the two (else 400).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct DeleteIn {
    pub source: Option<String>,
    /// ISO-8601; without it the dry run fixes "until now" (app clock) and returns it in each
    /// removal's `before`, so the confirmation deletes exactly what was previewed.
    pub before: Option<String>,
}

impl From<gitprompt_store::housekeeping::Report> for ReportOut {
    fn from(r: gitprompt_store::housekeeping::Report) -> Self {
        let (rows, bytes) = (r.rows(), r.bytes());
        let removals = r
            .removals
            .into_iter()
            .map(|m| RemovalOut { source: m.source, event: m.event, rows: m.rows, bytes: m.bytes, why: m.why, before: m.before })
            .collect();
        ReportOut { removals, rows, bytes, size_before: r.size_before, applied: r.size_after.is_some(), size_after: r.size_after }
    }
}
