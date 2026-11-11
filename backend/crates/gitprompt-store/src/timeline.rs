//! Reads for stories and search (ADR-0016): prompts as the index sees them, in time order, with
//! their typed text and outcome, plus the few turn events a story shows.

use std::collections::BTreeMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use gitprompt_core::spec::ToolKind;
use gitprompt_core::{Json, Result};

use crate::Store;
use crate::decode::{self, text_at};
use crate::error::{DbResult, failure};
use crate::filter::{Outcome, PromptFilter};
use crate::index::{IndexRow, index_row, row_columns, tool_kind};
use crate::sql::filter::{conditions, outcome};
use crate::sql::{Sql, int, json_list};

/// A prompt as stories and search see it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexedPrompt {
    #[serde(flatten)]
    pub row: IndexRow,
    pub outcome: Outcome,
}

/// A finished tool call or a compaction of a turn, with only what a story shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnEvent {
    pub prompt_id: String,
    pub event: String,
    pub received_at: String,
    pub tool: Option<String>,
    /// The tool input when it is an object.
    pub tool_input: Option<Json>,
    /// `tool_input.description`, else `tool_input.task_name`.
    pub description: Option<String>,
    pub subagent_type: Option<String>,
    /// `tool_input.prompt`, else `tool_input.message`, clipped to `clip` characters.
    pub prompt: Option<String>,
}

impl Store {
    /// The prompts the filter keeps, oldest first unless `newest_first` (by `received_at`, then id).
    pub fn timeline(&self, filter: &PromptFilter, limit: Option<usize>, newest_first: bool) -> Result<Vec<IndexedPrompt>> {
        let running_since = self.running_since();
        let direction = if newest_first { "desc" } else { "asc" };
        let mut query = indexed(&running_since)
            .push(" where ")
            .push(conditions(filter, &running_since)?)
            .push(format!(" order by i.received_at {direction}, i.id {direction}"));
        if let Some(limit) = limit {
            query = query.push(" limit ").bind(int(limit));
        }
        Ok(self.db().read(|conn| decode::all(conn, query.text(), query.params(), indexed_prompt))?)
    }

    pub fn timeline_one(&self, id: i64) -> Result<Option<IndexedPrompt>> {
        let query = indexed(&self.running_since()).push(" where i.id = ").bind(id);
        Ok(self.db().read(|conn| decode::first(conn, query.text(), query.params(), indexed_prompt))?)
    }

    pub fn count(&self, filter: &PromptFilter) -> Result<i64> {
        let query = Sql::new("select count(*) from prompt_index i where ").push(conditions(filter, &self.running_since())?);
        Ok(self.db().read(|conn| Ok(conn.prepare_cached(query.text())?.query_row(query.params(), |row| row.get(0))?))?)
    }

    /// Tool calls per kind of each prompt's turn.
    pub fn tool_kinds(&self, ids: &[i64]) -> Result<BTreeMap<i64, BTreeMap<ToolKind, i64>>> {
        let ids = json_list(ids)?;
        let rows = self.db().read(|conn| {
            let select = "select id, kind, sum(calls) n from prompt_index_tool where id in (select value from json_each(?)) group by 1, 2";
            decode::all(conn, select, [ids], |row| {
                Ok((row.get::<_, i64>("id")?, tool_kind(&row.get::<_, String>("kind")?), row.get::<_, i64>("n")?))
            })
        })?;
        let mut out: BTreeMap<i64, BTreeMap<ToolKind, i64>> = BTreeMap::new();
        for (id, kind, calls) in rows {
            *out.entry(id).or_default().entry(kind).or_default() += calls;
        }
        Ok(out)
    }

    /// `PostToolUse` and `PreCompact` events of these turns, oldest first.
    pub fn turn_events(&self, prompt_ids: &[String], clip: usize) -> Result<Vec<TurnEvent>> {
        let turns = json_list(prompt_ids)?;
        Ok(self.db().read(|conn| decode::all(conn, TURN_EVENTS, (int(clip), turns), turn_event))?)
    }
}

/// Index rows with their typed text and outcome, from `prompt_index i`.
fn indexed(running_since: &str) -> Sql {
    Sql::new(format!("select {}, ", row_columns())).push(outcome(running_since)).push(" outcome from prompt_index i")
}

fn indexed_prompt(row: &Row<'_>) -> DbResult<IndexedPrompt> {
    let name: String = row.get("outcome")?;
    let outcome = Outcome::from_name(&name).ok_or_else(|| failure(format!("unknown outcome {name}")))?;
    Ok(IndexedPrompt { row: index_row(row)?, outcome })
}

/// One row per tool call or compaction of a turn, with only the fields a story shows: the tool's
/// input (the files a write tool touched), and what a subagent was asked (Claude Code's Agent tool,
/// Codex's `spawn_agent`). `?1`: characters of the brief; `?2`: the turns, as a JSON array.
const TURN_EVENTS: &str = "select prompt_id, event, received_at, json_extract(raw, '$.tool_name') tool,
  case json_type(raw, '$.tool_input') when 'object' then json_extract(raw, '$.tool_input') end tool_input,
  coalesce(json_extract(raw, '$.tool_input.description'), json_extract(raw, '$.tool_input.task_name')) description,
  json_extract(raw, '$.tool_input.subagent_type') subagent_type,
  substr(coalesce(json_extract(raw, '$.tool_input.prompt'), json_extract(raw, '$.tool_input.message')), 1, ?1) prompt
from event where event in ('PostToolUse', 'PreCompact') and prompt_id in (select value from json_each(?2)) order by id";

fn turn_event(row: &Row<'_>) -> DbResult<TurnEvent> {
    let tool_input: Option<String> = row.get("tool_input")?;
    Ok(TurnEvent {
        prompt_id: text_at(row, "prompt_id")?.unwrap_or_default(),
        event: text_at(row, "event")?.unwrap_or_default(),
        received_at: row.get("received_at")?,
        tool: text_at(row, "tool")?,
        tool_input: tool_input.map(|input| serde_json::from_str(&input)).transpose()?,
        description: text_at(row, "description")?,
        subagent_type: text_at(row, "subagent_type")?,
        prompt: text_at(row, "prompt")?,
    })
}
