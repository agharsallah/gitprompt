//! Writing one prompt's [`Entry`] into the index tables, through statements prepared once per batch.

use rusqlite::{CachedStatement, Connection};

use super::Entry;
use super::codec::{COLUMNS, USAGE, values};
use crate::error::DbResult;

/// The insert statement of every index table.
pub(super) struct Inserts<'c> {
    row: CachedStatement<'c>,
    model: CachedStatement<'c>,
    tool: CachedStatement<'c>,
    work: CachedStatement<'c>,
    leak: CachedStatement<'c>,
    text: CachedStatement<'c>,
}

impl<'c> Inserts<'c> {
    pub(super) fn prepare(conn: &'c Connection) -> DbResult<Self> {
        let row_sql = format!("insert into prompt_index ({}) values ({})", COLUMNS.join(", "), vec!["?"; COLUMNS.len()].join(", "));
        Ok(Inserts {
            row: conn.prepare_cached(&row_sql)?,
            model: conn.prepare_cached(&format!("insert into prompt_index_model (id, model, {USAGE}) values (?, ?, ?, ?, ?, ?, ?, ?)"))?,
            tool: conn.prepare_cached("insert into prompt_index_tool (id, tool, kind, calls) values (?, ?, ?, ?)")?,
            work: conn.prepare_cached(&format!(
                "insert into prompt_index_work (id, work, query_source, {USAGE}) values (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ))?,
            leak: conn
                .prepare_cached("insert into prompt_index_leak (id, finding, value_key, kind, place, masked) values (?, ?, ?, ?, ?, ?)")?,
            text: conn.prepare_cached("insert into prompt_fts (rowid, typed_text) values (?, ?)")?,
        })
    }

    /// Insert `entry` into every table; `kinds` are its tools' kinds as stored, in order.
    pub(super) fn entry(&mut self, entry: &Entry, kinds: &[String]) -> DbResult<()> {
        let id = entry.row.id;
        self.row.execute(rusqlite::params_from_iter(values(&entry.row)))?;
        for m in &entry.models {
            let u = &m.usage;
            self.model.execute((
                id,
                &m.model,
                u.api_requests,
                u.cost_usd,
                u.input_tokens,
                u.output_tokens,
                u.cache_read_tokens,
                u.cache_creation_tokens,
            ))?;
        }
        for (t, kind) in entry.tools.iter().zip(kinds) {
            self.tool.execute((id, &t.tool, kind, t.calls))?;
        }
        for w in &entry.work {
            let u = &w.usage;
            self.work.execute((
                id,
                &w.work,
                &w.query_source,
                u.api_requests,
                u.cost_usd,
                u.input_tokens,
                u.output_tokens,
                u.cache_read_tokens,
                u.cache_creation_tokens,
            ))?;
        }
        for l in &entry.leaks {
            self.leak.execute((id, &l.finding, &l.value_key, &l.kind, &l.place, &l.masked))?;
        }
        if let Some(typed) = entry.row.typed_text.as_deref().filter(|t| !t.is_empty()) {
            self.text.execute((id, typed))?;
        }
        Ok(())
    }
}
