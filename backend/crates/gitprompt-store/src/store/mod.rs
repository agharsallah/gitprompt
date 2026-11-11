//! The store: prompts, events and annotations, plus housekeeping primitives. Port of
//! storage/store.py; the methods live in submodules by kind (writes, reads, edits, maintenance).

mod edits;
mod maintenance;
mod reads;
mod writes;

use std::path::Path;
use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use gitprompt_core::{Json, JsonMap, Result};

use crate::db::Db;
use crate::index::PromptIndex;
use crate::time;

/// Values per `in (...)` query of the Python store. The Rust store binds a list as one JSON array
/// (`json_each(?)`), so lists of any length are read in one query; kept for the API.
pub const CHUNK: usize = 500;

/// One stored payload, for the field catalog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub source: String,
    /// The event name, `"?"` when unknown; `"context (at ingest)"` for GitPrompt's own context
    /// (with source `"gitprompt"`).
    pub event: String,
    pub data: Json,
}

/// Per source: prompts, prompts without text (drift), events, last activity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceStats {
    pub total: u64,
    pub unmapped: u64,
    pub events: u64,
    pub last_at: Option<String>,
}

/// Rows received in one UTC hour (`YYYY-MM-DDTHH`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HourCount {
    pub source: String,
    pub hour: String,
    pub rows: u64,
}

/// Stored events of one (source, event): rows and approximate bytes of stored JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventUsage {
    pub source: String,
    pub event: Option<String>,
    pub rows: u64,
    pub bytes: u64,
}

/// A new event to store in a batch.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEvent {
    pub source: String,
    pub fields: JsonMap,
    pub raw: Json,
}

/// The store's clock: `received_at` of new rows and the 1-hour `running` window of
/// [`crate::Outcome`]. New: the Python store read the system clock and SQLite's `now`; tests pin it
/// with [`Store::with_clock`].
pub type Clock = Arc<dyn Fn() -> Timestamp + Send + Sync>;

/// Prompts, events and annotations in SQLite. `Send + Sync`: share one store between threads by
/// reference (or `Arc`); every method takes `&self`.
pub struct Store {
    db: Db,
    clock: Clock,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Open (creating it, its parent directory with mode 0700, and the schema) a database file of
    /// mode 0600 in WAL mode. Opening a database the Python daemon wrote keeps every row.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::with_db(Db::open(path)?))
    }

    /// A private in-memory database (tests, previews).
    pub fn open_in_memory() -> Result<Self> {
        Ok(Self::with_db(Db::open_in_memory()?))
    }

    fn with_db(db: Db) -> Self {
        Store { db, clock: Arc::new(Timestamp::now) }
    }

    /// Run `f` all-or-nothing: an `Err` rolls back every write it made. Every store call `f` makes
    /// on the store it is given joins the transaction (nested transactions become savepoints);
    /// writes from other threads wait until it ends.
    #[track_caller]
    pub fn transaction<T>(&self, f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        self.db.transaction(|| f(self))
    }

    /// How the database writer has been shared since this store opened: entries, the ones that
    /// waited or kept it too long, and the longest wait and stay (`warn` logs name where).
    pub fn writer_stats(&self) -> crate::WriterStats {
        self.db.writer_stats()
    }

    /// Use `clock` instead of the system clock for `received_at` of new rows and values derived on
    /// read (the `running` outcome window). New; the daemon keeps the system clock.
    #[must_use]
    pub fn with_clock(self, clock: Clock) -> Self {
        Store { clock, ..self }
    }

    pub fn index(&self) -> PromptIndex<'_> {
        PromptIndex { store: self }
    }

    /// The prompt id given to a prompt whose tool sends none: `<source>#<id>`.
    pub fn turn_name(source: &str, id: i64) -> String {
        format!("{source}#{id}")
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    /// Now, as a `received_at`.
    pub(crate) fn now(&self) -> String {
        time::iso((self.clock)())
    }

    /// The `received_at` from which a turn without an end is still `running` (one hour ago).
    pub(crate) fn running_since(&self) -> String {
        let now = (self.clock)();
        time::iso(time::before(now, SignedDuration::from_hours(1)).unwrap_or(Timestamp::MIN))
    }
}
