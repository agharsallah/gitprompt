//! The prompt index: a cache of derived values so filters and statistics run in SQL (ADR-0012). It
//! can be dropped any time and is rebuilt from the raw rows (gitprompt-insights computes the values,
//! this module only stores them).

mod codec;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use gitprompt_core::Result;
use gitprompt_core::spec::ToolKind;

use crate::decode;
use crate::error::{DbResult, failure};
use crate::schema;
use crate::sql::json_list;
use codec::{COLUMNS, USAGE, tool_kinds, usage, values};
pub(crate) use codec::{index_row, row_columns, tool_kind};

/// Requests, tokens and cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub api_requests: i64,
    pub cost_usd: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
}

/// One `prompt_index` row (+ the typed text, which goes to the FTS table only).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IndexRow {
    pub id: i64,
    pub prompt_id: Option<String>,
    pub source: String,
    pub session: Option<String>,
    /// git root at ingest, else the working directory.
    pub repo: Option<String>,
    /// owner/name of the origin remote.
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub title: Option<String>,
    pub received_at: String,
    /// Local calendar day, YYYY-MM-DD.
    pub day: String,
    pub hour: u8,
    /// ISO weekday, 1 = Monday.
    pub weekday: u8,
    pub origin: String,
    pub chars: i64,
    pub text_hash: Option<String>,
    pub slash_command: Option<String>,
    pub code_blocks: i64,
    pub urls: i64,
    pub mentions: i64,
    pub secrets: i64,
    #[serde(flatten)]
    pub usage: UsageTotals,
    pub api_time_s: f64,
    pub api_errors: i64,
    pub response_s: Option<f64>,
    /// Stop, `StopFailure`, Interrupt, or none.
    pub ended_by: Option<String>,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub tool_time_s: f64,
    pub subagent_tool_calls: i64,
    pub subagents: i64,
    pub files_written: i64,
    pub git_commits: i64,
    pub compactions: i64,
    pub denials: i64,
    pub rejections: i64,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub tool_version: Option<String>,
    pub reasoning_tokens: i64,
    pub answered: bool,
    pub surface: Option<String>,
    /// Goes to `prompt_fts` (trigram), not `prompt_index`.
    pub typed_text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: Option<String>,
    #[serde(flatten)]
    pub usage: UsageTotals,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCalls {
    pub tool: Option<String>,
    pub kind: ToolKind,
    pub calls: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkUsage {
    /// main, subagent, system
    pub work: String,
    pub query_source: Option<String>,
    #[serde(flatten)]
    pub usage: UsageTotals,
}

/// Everything the index stores about one prompt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub row: IndexRow,
    pub models: Vec<ModelUsage>,
    pub tools: Vec<ToolCalls>,
    pub work: Vec<WorkUsage>,
}

/// A telemetry metric data point of a session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionMetric {
    pub event_id: i64,
    pub session: Option<String>,
    pub received_at: String,
    pub name: String,
    pub value: f64,
}

/// A slash command telemetry saw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCommand {
    pub event_id: i64,
    pub session: Option<String>,
    pub received_at: String,
    pub command: String,
    pub source: Option<String>,
}

/// The index tables of a store.
#[derive(Debug)]
pub struct PromptIndex<'a> {
    pub(crate) store: &'a crate::Store,
}

impl PromptIndex<'_> {
    /// Create the tables (idempotent). Needs FTS5 with the trigram tokenizer.
    pub fn create(&self) -> Result<()> {
        Ok(self.store.db().write(schema::create_index)?)
    }

    /// Forget everything (the index is a cache); `create` brings back empty tables.
    pub fn drop_all(&self) -> Result<()> {
        Ok(self.store.db().atomic(|conn| {
            for table in schema::INDEX_TABLES {
                conn.execute_batch(&format!("drop table if exists {table}"))?;
            }
            Ok(())
        })?)
    }

    /// Mark the whole index stale: the next `ensure` rebuilds it.
    pub fn invalidate(&self) -> Result<()> {
        Ok(self.store.db().write(|conn| {
            conn.prepare_cached("delete from index_meta where key = 'rules_version'")?.execute([])?;
            Ok(())
        })?)
    }

    /// The rules version the rows were computed with.
    pub fn version(&self) -> Result<Option<String>> {
        Ok(self.store.db().read(|conn| meta(conn, RULES_VERSION))?)
    }

    pub fn set_version(&self, value: &str) -> Result<()> {
        Ok(self.store.db().write(|conn| set_meta(conn, RULES_VERSION, value))?)
    }

    /// Replace the rows of these prompts (index, model, tool, work, FTS), in one transaction.
    /// Returns how many.
    pub fn upsert(&self, entries: &[Entry]) -> Result<usize> {
        let ids: Vec<i64> = entries.iter().map(|e| e.row.id).collect();
        let rows = entries.iter().map(|e| Ok((e, tool_kinds(&e.tools)?))).collect::<DbResult<Vec<_>>>()?;
        Ok(self.store.db().atomic(|conn| {
            delete(conn, &ids)?;
            let mut row = conn.prepare_cached(&format!(
                "insert into prompt_index ({}) values ({})",
                COLUMNS.join(", "),
                vec!["?"; COLUMNS.len()].join(", ")
            ))?;
            let mut model =
                conn.prepare_cached(&format!("insert into prompt_index_model (id, model, {USAGE}) values (?, ?, ?, ?, ?, ?, ?, ?)"))?;
            let mut tool = conn.prepare_cached("insert into prompt_index_tool (id, tool, kind, calls) values (?, ?, ?, ?)")?;
            let mut work = conn.prepare_cached(&format!(
                "insert into prompt_index_work (id, work, query_source, {USAGE}) values (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ))?;
            let mut text = conn.prepare_cached("insert into prompt_fts (rowid, typed_text) values (?, ?)")?;
            for (entry, kinds) in &rows {
                let id = entry.row.id;
                row.execute(rusqlite::params_from_iter(values(&entry.row)))?;
                for m in &entry.models {
                    let u = &m.usage;
                    model.execute((
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
                    tool.execute((id, &t.tool, kind, t.calls))?;
                }
                for w in &entry.work {
                    let u = &w.usage;
                    work.execute((
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
                if let Some(typed) = entry.row.typed_text.as_deref().filter(|t| !t.is_empty()) {
                    text.execute((id, typed))?;
                }
            }
            Ok(entries.len())
        })?)
    }

    pub fn delete(&self, ids: &[i64]) -> Result<()> {
        Ok(self.store.db().atomic(|conn| delete(conn, ids))?)
    }

    /// The last event id read for session telemetry (0 at first).
    pub fn telemetry_after(&self) -> Result<i64> {
        let stored = self.store.db().read(|conn| meta(conn, TELEMETRY_EVENT_ID))?;
        stored.map_or(Ok(0), |id| id.trim().parse().map_err(|_| failure(format!("stored {TELEMETRY_EVENT_ID} is not an id: {id}")).into()))
    }

    pub fn add_telemetry(&self, last_event: i64, metrics: &[SessionMetric], commands: &[SessionCommand]) -> Result<()> {
        Ok(self.store.db().atomic(|conn| {
            let mut metric = conn.prepare_cached("insert or replace into session_metric values (?, ?, ?, ?, ?)")?;
            for m in metrics {
                metric.execute((m.event_id, &m.session, &m.received_at, &m.name, m.value))?;
            }
            let mut command = conn.prepare_cached("insert or replace into session_command values (?, ?, ?, ?, ?)")?;
            for c in commands {
                command.execute((c.event_id, &c.session, &c.received_at, &c.command, &c.source))?;
            }
            set_meta(conn, TELEMETRY_EVENT_ID, &last_event.to_string())
        })?)
    }

    /// Note that these turns changed (cheap: done on every ingest). Idempotent.
    pub fn mark(&self, prompt_ids: &[String]) -> Result<()> {
        let turns = json_list(prompt_ids)?;
        Ok(self.store.db().write(|conn| {
            conn.prepare_cached("insert or ignore into index_dirty (prompt_id) select value from json_each(?)")?.execute([turns])?;
            Ok(())
        })?)
    }

    /// The marked turns, unmarked.
    pub fn take_marked(&self) -> Result<Vec<String>> {
        Ok(self.store.db().write(|conn| decode::all(conn, "delete from index_dirty returning prompt_id", [], |row| Ok(row.get(0)?)))?)
    }

    /// How many changed turns wait for the next `ensure` (what [`Self::take_marked`] would take),
    /// without unmarking them.
    pub fn marked_count(&self) -> Result<u64> {
        Ok(self.store.db().read(|conn| {
            let count: i64 = conn.prepare_cached("select count(*) from index_dirty")?.query_row([], |row| row.get(0))?;
            Ok(u64::try_from(count).unwrap_or(0))
        })?)
    }

    /// Prompts stored after the newest indexed one (an index lookup, not a scan). Ported as is: a
    /// gap below the newest indexed prompt is not reported (ingest indexes in order).
    pub fn unindexed(&self) -> Result<Vec<i64>> {
        Ok(self.store.db().read(|conn| {
            let select = "select id from prompt where id > (select coalesce(max(id), 0) from prompt_index) order by id";
            decode::all(conn, select, [], |row| Ok(row.get(0)?))
        })?)
    }

    /// The stored index row of a prompt (tests, the prompt API's facts), with `typed_text` read
    /// back from `prompt_fts`.
    pub fn row(&self, id: i64) -> Result<Option<IndexRow>> {
        Ok(self.store.db().read(|conn| row_of(conn, id))?)
    }

    /// Everything stored for a prompt: its row plus its model, tool and work rows (in insert order).
    pub fn entry(&self, id: i64) -> Result<Option<Entry>> {
        Ok(self.store.db().read(|conn| {
            let Some(row) = row_of(conn, id)? else {
                return Ok(None);
            };
            let models = &format!("select model, {USAGE} from prompt_index_model where id = ? order by rowid");
            let models = decode::all(conn, models, [id], |r| Ok(ModelUsage { model: r.get("model")?, usage: usage(r)? }))?;
            let tools = "select tool, kind, calls from prompt_index_tool where id = ? order by rowid";
            let tools = decode::all(conn, tools, [id], |r| {
                Ok(ToolCalls { tool: r.get("tool")?, kind: tool_kind(&r.get::<_, String>("kind")?), calls: r.get("calls")? })
            })?;
            let work = &format!("select work, query_source, {USAGE} from prompt_index_work where id = ? order by rowid");
            let work = decode::all(conn, work, [id], |r| {
                Ok(WorkUsage { work: r.get("work")?, query_source: r.get("query_source")?, usage: usage(r)? })
            })?;
            Ok(Some(Entry { row, models, tools, work }))
        })?)
    }
}

const RULES_VERSION: &str = "rules_version";
pub(crate) const TELEMETRY_EVENT_ID: &str = "telemetry_event_id";

fn row_of(conn: &Connection, id: i64) -> DbResult<Option<IndexRow>> {
    let select = &format!("select {} from prompt_index i where i.id = ?", row_columns());
    decode::first(conn, select, [id], index_row)
}

fn delete(conn: &Connection, ids: &[i64]) -> DbResult<()> {
    let ids = json_list(ids)?;
    for (table, column) in [
        ("prompt_index", "id"),
        ("prompt_index_model", "id"),
        ("prompt_index_tool", "id"),
        ("prompt_index_work", "id"),
        ("prompt_fts", "rowid"),
    ] {
        conn.prepare_cached(&format!("delete from {table} where {column} in (select value from json_each(?))"))?.execute([&ids])?;
    }
    Ok(())
}

fn meta(conn: &Connection, key: &str) -> DbResult<Option<String>> {
    decode::first(conn, "select value from index_meta where key = ?", [key], |row| Ok(row.get(0)?))
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> DbResult<()> {
    conn.prepare_cached("insert or replace into index_meta (key, value) values (?, ?)")?.execute((key, value))?;
    Ok(())
}
