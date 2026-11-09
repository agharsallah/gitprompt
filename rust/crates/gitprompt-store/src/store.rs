//! The store: prompts, events and annotations, plus housekeeping primitives.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use jiff::Timestamp;

use serde::{Deserialize, Serialize};

use gitprompt_core::{Json, JsonMap, Result};

use crate::filter::{Outcome, PromptFilter};
use crate::index::PromptIndex;
use crate::rows::{Annotation, EventRow, PromptRow, PromptView};

/// Values per `in (...)` query.
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

/// The clock values derived on read use (the 1-hour `running` window of [`Outcome`]). New: the
/// Python store read SQLite's `now`; tests pin it with [`Store::with_clock`].
pub type Clock = Arc<dyn Fn() -> Timestamp + Send + Sync>;

pub struct Store {
    _private: (),
}

impl Store {
    /// Open (creating it, its parent directory with mode 0700, and the schema) a database file of
    /// mode 0600 in WAL mode. Opening a database the Python daemon wrote keeps every row.
    pub fn open(_path: &Path) -> Result<Self> {
        todo!()
    }

    /// A private in-memory database (tests, previews).
    pub fn open_in_memory() -> Result<Self> {
        todo!()
    }

    /// Run `f` all-or-nothing: an `Err` rolls back every write it made.
    pub fn transaction<T>(&self, _f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        todo!()
    }

    /// Use `clock` instead of the system clock for values derived on read (the `running` outcome
    /// window). New; the daemon keeps the system clock.
    pub fn with_clock(self, _clock: Clock) -> Self {
        todo!()
    }

    pub fn index(&self) -> PromptIndex<'_> {
        todo!()
    }

    // --- writes from the receiver ---

    /// Insert a prompt; `None` if this source already sent the same `prompt_id` (dedup). Prompts
    /// without a `prompt_id` never collide. `prompt` is `fields.prompt` when it is a string.
    pub fn add_prompt(&self, _source: &str, _fields: &JsonMap, _context: &JsonMap, _raw: &Json) -> Result<Option<PromptRow>> {
        todo!()
    }

    /// [`Self::add_prompt`] with an explicit `received_at` (UTC ISO-8601, stored as given) instead of
    /// now. New: imports, and tests that need prompts at known times.
    pub fn add_prompt_at(
        &self,
        _source: &str,
        _fields: &JsonMap,
        _context: &JsonMap,
        _raw: &Json,
        _received_at: &str,
    ) -> Result<Option<PromptRow>> {
        todo!()
    }

    /// Insert an event; `event`, `session_id`, `prompt_id` columns come from `fields`.
    pub fn add_event(&self, _source: &str, _fields: &JsonMap, _raw: &Json) -> Result<EventRow> {
        todo!()
    }

    /// [`Self::add_event`] with an explicit `received_at` (UTC ISO-8601, stored as given). New.
    pub fn add_event_at(&self, _source: &str, _fields: &JsonMap, _raw: &Json, _received_at: &str) -> Result<EventRow> {
        todo!()
    }

    /// Insert many events in one transaction (an OTLP batch): one fsync instead of one per row.
    pub fn add_events(&self, _events: &[NewEvent]) -> Result<Vec<EventRow>> {
        todo!()
    }

    // --- joining events to prompts without a prompt id (spec `join: session`) ---

    /// The prompt id given to a prompt whose tool sends none: `<source>#<id>`.
    pub fn turn_name(source: &str, id: i64) -> String {
        format!("{source}#{id}")
    }

    /// Store `turn_name` as the prompt's `fields.prompt_id`; returns it.
    pub fn name_turn(&self, _id: i64, _source: &str) -> Result<String> {
        todo!()
    }

    /// The prompt id of the latest prompt of this source and session.
    pub fn latest_turn(&self, _source: &str, _session_id: &str) -> Result<Option<String>> {
        todo!()
    }

    /// Redo the session join over everything stored for a source (after a remap): name prompts that
    /// have no prompt id, attach each event without one to the latest prompt of its session received
    /// at or before it, and copy that id into the event's fields.
    pub fn rejoin(&self, _source: &str) -> Result<()> {
        todo!()
    }

    // --- reads ---

    /// Newest prompts first, narrowed by the filter, at most `limit`.
    pub fn search(&self, _filter: &PromptFilter, _limit: usize) -> Result<Vec<PromptView>> {
        todo!()
    }

    /// How each prompt's turn ended (from the prompt index).
    pub fn outcomes(&self, _ids: &[i64]) -> Result<HashMap<i64, Outcome>> {
        todo!()
    }

    /// Stored prompt rows by id (any number of ids; unknown ids are skipped).
    pub fn prompts(&self, _ids: &[i64]) -> Result<Vec<PromptRow>> {
        todo!()
    }

    /// Row ids of the prompts these prompt_ids belong to (any source).
    pub fn prompts_of_turns(&self, _prompt_ids: &[String]) -> Result<Vec<i64>> {
        todo!()
    }

    /// Every prompt row id, ascending.
    pub fn prompt_ids(&self) -> Result<Vec<i64>> {
        todo!()
    }

    pub fn get(&self, _id: i64) -> Result<Option<PromptView>> {
        todo!()
    }

    /// Events of these turns, oldest first.
    pub fn events_for_prompts(&self, _prompt_ids: &[String]) -> Result<Vec<EventRow>> {
        todo!()
    }

    /// Events newer than `event_id` whose name starts with `prefix` or is one of `names`, oldest first.
    pub fn events_after(&self, _event_id: i64, _prefix: &str, _names: &[&str]) -> Result<Vec<EventRow>> {
        todo!()
    }

    pub fn events_for_session(&self, _session_id: &str) -> Result<Vec<EventRow>> {
        todo!()
    }

    /// The most recent payloads (up to `limit` events and `limit` prompts, each prompt followed by its
    /// ingest context), for the field catalog.
    pub fn samples(&self, _limit: usize) -> Result<Vec<Sample>> {
        todo!()
    }

    /// Per source: prompt total, prompts with no text, event count, last activity (prompt or event).
    pub fn source_stats(&self) -> Result<BTreeMap<String, SourceStats>> {
        todo!()
    }

    /// Rows (prompts and events) per source and UTC hour received at or after `since`.
    pub fn hourly(&self, _since: &str) -> Result<Vec<HourCount>> {
        todo!()
    }

    // --- housekeeping ---

    /// Stored events per (source, event), largest first; `before`: only rows received earlier.
    pub fn event_usage(&self, _before: Option<&str>) -> Result<Vec<EventUsage>> {
        todo!()
    }

    /// Delete stored events of one kind (`event` `None` = events without a name), optionally only
    /// those received before `before`. Prompts and annotations are never touched.
    pub fn delete_events(&self, _source: &str, _event: Option<&str>, _before: Option<&str>) -> Result<u64> {
        todo!()
    }

    /// Bytes the database occupies (page_count × page_size).
    pub fn size(&self) -> Result<u64> {
        todo!()
    }

    /// Rewrite the file without free pages, then fold the WAL back in.
    pub fn vacuum(&self) -> Result<()> {
        todo!()
    }

    /// Copy the database to `dest` (online backup API; consistent while ingest continues). New.
    pub fn backup_to(&self, _dest: &Path) -> Result<()> {
        todo!()
    }

    // --- edits ---

    pub fn annotation(&self, _id: i64) -> Result<Annotation> {
        todo!()
    }

    pub fn annotate(&self, _id: i64, _data: &Annotation) -> Result<()> {
        todo!()
    }

    /// Apply `edit` to a prompt's fields, context and raw, and to its turn's events' fields and raw
    /// (e.g. to hide a secret), in one transaction. Returns rows changed (0 for an unknown id).
    pub fn rewrite(&self, _id: i64, _edit: &dyn Fn(&Json) -> Json) -> Result<u64> {
        todo!()
    }

    /// Re-run `mapper(raw) -> fields` over every stored payload of a source (after a spec fix);
    /// event columns follow the new fields. Returns rows remapped.
    pub fn remap(&self, _source: &str, _mapper: &dyn Fn(&Json) -> JsonMap) -> Result<u64> {
        todo!()
    }
}
