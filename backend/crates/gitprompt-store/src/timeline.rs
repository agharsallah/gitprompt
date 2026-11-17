//! Reads for stories and search (ADR-0016): prompts as the index sees them, in time order, with
//! their typed text and outcome, plus the few turn events a story shows.

use std::collections::BTreeMap;

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use gitprompt_core::spec::{Origin, ToolKind};
use gitprompt_core::{Json, Result};

use crate::Store;
use crate::decode::{self, text_at};
use crate::error::{DbResult, failure};
use crate::filter::{Outcome, PromptFilter};
use crate::index::{IndexRow, index_row, row_columns, tool_kind};
use crate::sql::filter::{conditions, name, outcome};
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

/// What names a session when you have not: the title its tool reported and the first prompts you
/// wrote in it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opener {
    /// The first title any prompt of the session carries.
    pub title: Option<String>,
    /// The typed text of its first prompts of the origins asked for, oldest first, each cut at
    /// [`OPENER_CHARS`] characters; prompts without text are skipped.
    pub typed: Vec<String>,
}

/// Where a session ran and when it last did anything: what a resume command needs, and whether
/// the session is still going.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Home {
    /// The first prompt's source (the connector id).
    pub source: Option<String>,
    /// The first `cwd` any event of the session carries, else the first prompt's.
    pub cwd: Option<String>,
    /// When the latest event or prompt of the session was received.
    pub last_event_at: Option<String>,
}

/// Characters of each typed text [`Store::session_openers`] returns: room to drop a slash command
/// or a path and still fill a label.
pub const OPENER_CHARS: usize = 400;

const OPENER_TITLES: &str = "select s.value session, (select title from prompt_index where session = s.value and coalesce(title, '') <> '' \
    order by id limit 1) title from json_each(?1) s";

const HOMES: &str = "select s.value session,
  (select source from prompt_index where session = s.value order by id limit 1) source,
  coalesce(
    (select json_extract(fields, '$.cwd') from event where session_id = s.value and coalesce(json_extract(fields, '$.cwd'), '') <> '' order by id limit 1),
    (select json_extract(p.fields, '$.cwd') from prompt_index i join prompt p on p.id = i.id
       where i.session = s.value and coalesce(json_extract(p.fields, '$.cwd'), '') <> '' order by i.id limit 1)
  ) cwd,
  (select received_at from event where session_id = s.value order by id desc limit 1) last_event,
  (select received_at from prompt_index where session = s.value order by id desc limit 1) last_prompt
from json_each(?1) s";

const OPENER_TEXTS: &str = "select session, text from (
  select i.session session, i.id id, substr(f.typed_text, 1, ?4) text, row_number() over (partition by i.session order by i.id) n
  from prompt_index i join prompt_fts f on f.rowid = i.id
  where i.session in (select value from json_each(?1)) and i.origin in (select value from json_each(?2)) and f.typed_text <> ''
) where n <= ?3 order by session, id";

impl Store {
    /// The [`Opener`] of each of these sessions: its title, and the first `per_session` typed
    /// texts of prompts with one of `origins`.
    pub fn session_openers(&self, sessions: &[String], origins: &[Origin], per_session: usize) -> Result<BTreeMap<String, Opener>> {
        let origins = origins.iter().map(name).collect::<DbResult<Vec<_>>>()?;
        let (ids, origins) = (json_list(sessions)?, json_list(&origins)?);
        let (titles, texts) = self.db().read(|conn| {
            let titles = decode::all(conn, OPENER_TITLES, [&ids], |row| Ok((row.get::<_, String>("session")?, text_at(row, "title")?)))?;
            let texts = decode::all(conn, OPENER_TEXTS, (&ids, origins, int(per_session), int(OPENER_CHARS)), |row| {
                Ok((row.get::<_, String>("session")?, row.get::<_, String>("text")?))
            })?;
            Ok((titles, texts))
        })?;
        let mut out: BTreeMap<String, Opener> =
            titles.into_iter().map(|(session, title)| (session, Opener { title, typed: Vec::new() })).collect();
        for (session, text) in texts {
            out.entry(session).or_default().typed.push(text);
        }
        Ok(out)
    }

    /// The [`Home`] of each of these sessions, in one read; a session with no prompt has none.
    pub fn session_homes(&self, sessions: &[String]) -> Result<BTreeMap<String, Home>> {
        let ids = json_list(sessions)?;
        let homes = self.db().read(|conn| {
            decode::all(conn, HOMES, [&ids], |row| {
                let last_event_at = text_at(row, "last_event")?.max(text_at(row, "last_prompt")?);
                Ok((row.get::<_, String>("session")?, Home { source: text_at(row, "source")?, cwd: text_at(row, "cwd")?, last_event_at }))
            })
        })?;
        Ok(homes.into_iter().filter(|(_, home)| home.source.is_some()).collect())
    }

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
