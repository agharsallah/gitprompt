//! The schema. `store.sql` and `index.sql` are storage/store.py and storage/index.py `SCHEMA`
//! verbatim (placeholders expanded), so a database the Python daemon wrote opens with no migration.
//! `event_stats.sql` is the one addition of the Rust port: a trigger-maintained summary of the event
//! table, created and backfilled once on the first open of a database that lacks it.

use rusqlite::Connection;

use crate::error::{DbResult, failure};

/// prompt, event, annotation and the `prompt_view` view (recreated on every open, as Python does).
const STORE: &str = include_str!("store.sql");
/// The prompt index (storage/index.py): a cache that can be dropped and rebuilt.
const INDEX: &str = include_str!("index.sql");
/// Rows and bytes per (source, event), kept by triggers (see the file).
const EVENT_STATS: &str = include_str!("event_stats.sql");
/// Recomputes `event_stats` from the event table.
pub(crate) const REBUILD_EVENT_STATS: &str = "delete from event_stats; \
    insert into event_stats (source, event, rows, bytes, last_at) \
    select source, event, count(*), sum(length(cast(fields as blob)) + length(cast(raw as blob))), max(received_at) \
    from event group by source, event;";
/// Recomputes the `event_stats` rows of one source (`?1`).
pub(crate) const REBUILD_SOURCE_EVENT_STATS: &str = "delete from event_stats where source = ?1; \
    insert into event_stats (source, event, rows, bytes, last_at) \
    select source, event, count(*), sum(length(cast(fields as blob)) + length(cast(raw as blob))), max(received_at) \
    from event where source = ?1 group by source, event;";

/// Every table of the prompt index, for `drop_all`.
pub(crate) const INDEX_TABLES: [&str; 9] = [
    "index_meta",
    "index_dirty",
    "prompt_index",
    "prompt_index_model",
    "prompt_index_tool",
    "prompt_index_work",
    "prompt_fts",
    "session_metric",
    "session_command",
];

pub(crate) fn create(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(STORE)?;
    create_event_stats(conn)?;
    create_index(conn)
}

/// Create `event_stats` and its triggers, and fill it from the stored events, in one transaction so
/// no event written meanwhile (by another process) is missed. Does nothing once the table exists.
fn create_event_stats(conn: &Connection) -> DbResult<()> {
    let exists: bool =
        conn.query_row("select exists (select 1 from sqlite_master where type = 'table' and name = 'event_stats')", [], |row| row.get(0))?;
    if !exists {
        conn.execute_batch(&format!("begin immediate; {EVENT_STATS} {REBUILD_EVENT_STATS} commit;"))?;
    }
    Ok(())
}

/// Create the index tables (idempotent). Needs FTS5 with the trigram tokenizer (SQLite 3.34+).
pub(crate) fn create_index(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(INDEX)
        .map_err(|e| failure(format!("GitPrompt needs SQLite with FTS5 and the trigram tokenizer (have {}): {e}", rusqlite::version())))
}
