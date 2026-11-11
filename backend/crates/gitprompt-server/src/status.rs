//! What a GitPrompt install looks like from the inside: the answer of `GET /api/status`
//! ([`crate::Service::status`]) and the building blocks `gitprompt doctor` reuses without a running
//! daemon ([`counts`], [`index_status`], [`spool_backlog`]).
//!
//! Dashboard-safe: paths, counts and versions only, never the token or prompt text.

use serde::{Deserialize, Serialize};

use gitprompt_capture::spool::Spool;
use gitprompt_core::{Result, Settings};
use gitprompt_store::Store;

use crate::dashboard::DashboardInfo;
use crate::health::HealthOut;

/// This build's version (the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Which implementation serves: `"rust"` (the legacy Python daemon has no `/api/status`).
pub const BACKEND: &str = "rust";

/// `GET /api/status`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusOut {
    pub version: String,
    pub backend: String,
    /// When this server started, ISO-8601 UTC (the app clock).
    pub started_at: String,
    pub uptime_s: u64,
    /// The data directory.
    pub home: String,
    pub db_path: String,
    pub db_bytes: u64,
    pub prompts: u64,
    pub events: u64,
    pub index: IndexStatus,
    pub dashboard: DashboardInfo,
    /// Hook payloads spooled while the daemon was unreachable, per source with any.
    pub spool: Vec<SpoolBacklog>,
    /// Route latencies, database writer contention, memory and CPU, recent warnings (absent from
    /// older daemons: read as empty).
    #[serde(default)]
    pub health: HealthOut,
}

/// Stored rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub prompts: u64,
    pub events: u64,
}

/// The prompt index (filters, search, statistics) compared with the current rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStatus {
    /// The rules version the stored rows were computed with; `None` before the first index.
    pub rules_version: Option<String>,
    /// The rules version of the current specs, time zone and code.
    pub current_version: String,
    /// The stored version differs: the next read rebuilds the whole index.
    pub stale: bool,
    /// Prompts stored after the newest indexed one (indexed at the next read).
    pub unindexed: u64,
    /// Turns that received events since they were indexed (recomputed at the next read).
    pub dirty_turns: u64,
}

/// Spooled payloads of one source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpoolBacklog {
    pub source: String,
    pub payloads: u64,
}

/// Prompts and events stored, from the per-source statistics (no table scan of events).
pub fn counts(store: &Store) -> Result<Counts> {
    Ok(store
        .source_stats()?
        .values()
        .fold(Counts::default(), |total, s| Counts { prompts: total.prompts + s.total, events: total.events + s.events }))
}

/// The index state of `store` against `current_version` (`Indexer::version` of the current specs).
/// Read-only: nothing is indexed or unmarked.
pub fn index_status(store: &Store, current_version: &str) -> Result<IndexStatus> {
    let index = store.index();
    let rules_version = index.version()?;
    Ok(IndexStatus {
        stale: rules_version.as_deref() != Some(current_version),
        rules_version,
        current_version: current_version.to_owned(),
        unindexed: index.unindexed()?.len() as u64,
        dirty_turns: index.marked_count()?,
    })
}

/// Spooled payloads per source, sorted by source; sources with an empty queue are left out.
pub fn spool_backlog(settings: &Settings) -> Result<Vec<SpoolBacklog>> {
    let spool = Spool::of(settings);
    spool.sources()?.into_iter().map(|source| Ok(SpoolBacklog { payloads: spool.pending(&source)?.len() as u64, source })).collect()
}
