//! Reads of prompts and events.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;

use gitprompt_core::Result;

use super::{HourCount, Sample, SourceStats, Store};
use crate::decode::{self, count};
use crate::error::{DbResult, failure};
use crate::filter::{Outcome, PromptFilter};
use crate::rows::{EventRow, PromptRow, PromptView};
use crate::sql::{Sql, filter, int, json_list};

/// `prompt_view` (turn number and neighbours) of the prompts whose ids are the JSON array `?1`.
/// Only their sessions are numbered (found through the `prompt_session` index), not every prompt.
const VIEW_OF_IDS: &str = "select * from (
  select p.*, coalesce(a.data, '{}') annotation,
         row_number() over w turn,
         lag(p.received_at) over w prev_at,
         lead(json_extract(p.context, '$.git_head')) over w next_head
  from prompt p left join annotation a on a.id = p.id
  where (p.source, coalesce(json_extract(p.fields, '$.session_id'), '')) in (
    select source, coalesce(json_extract(fields, '$.session_id'), '') from prompt where id in (select value from json_each(?1)))
  window w as (partition by p.source, coalesce(json_extract(p.fields, '$.session_id'), '') order by p.id))
where id in (select value from json_each(?1))";

impl Store {
    /// Newest prompts first, narrowed by the filter, at most `limit`.
    pub fn search(&self, filter: &PromptFilter, limit: usize) -> Result<Vec<PromptView>> {
        let conditions = filter::conditions(filter, &self.running_since())?;
        let query = Sql::new("select i.id from prompt_index i where ").push(conditions).push(" order by i.id desc limit ").bind(int(limit));
        Ok(self.db().read(|conn| {
            let ids = decode::all(conn, query.text(), query.params(), |row| Ok(row.get(0)?))?;
            views(conn, &ids)
        })?)
    }

    pub fn get(&self, id: i64) -> Result<Option<PromptView>> {
        Ok(self.db().read(|conn| views(conn, &[id]))?.pop())
    }

    /// How each prompt's turn ended (from the prompt index; unindexed prompts are left out).
    pub fn outcomes(&self, ids: &[i64]) -> Result<HashMap<i64, Outcome>> {
        let query = Sql::new("select i.id, ")
            .push(filter::outcome(&self.running_since()))
            .push(" from prompt_index i where i.id in (select value from json_each(")
            .bind(json_list(ids)?)
            .push("))");
        Ok(self.db().read(|conn| {
            let rows = decode::all(conn, query.text(), query.params(), |row| {
                let name: String = row.get(1)?;
                let outcome = Outcome::from_name(&name).ok_or_else(|| failure(format!("unknown outcome {name}")))?;
                Ok((row.get(0)?, outcome))
            })?;
            Ok(rows.into_iter().collect())
        })?)
    }

    /// Stored prompt rows by id, ascending (any number of ids; unknown ids are skipped).
    pub fn prompts(&self, ids: &[i64]) -> Result<Vec<PromptRow>> {
        let ids = json_list(ids)?;
        Ok(self.db().read(|conn| {
            let select = "select * from prompt where id in (select value from json_each(?)) order by id";
            decode::all(conn, select, [ids], decode::prompt_row)
        })?)
    }

    /// Row ids of the prompts these `prompt_ids` belong to (any source), ascending.
    pub fn prompts_of_turns(&self, prompt_ids: &[String]) -> Result<Vec<i64>> {
        let turns = json_list(prompt_ids)?;
        Ok(self.db().read(|conn| {
            let select = "select id from prompt where json_extract(fields, '$.prompt_id') in (select value from json_each(?)) order by id";
            decode::all(conn, select, [turns], |row| Ok(row.get(0)?))
        })?)
    }

    /// Every prompt row id, ascending.
    pub fn prompt_ids(&self) -> Result<Vec<i64>> {
        Ok(self.db().read(|conn| decode::all(conn, "select id from prompt order by id", [], |row| Ok(row.get(0)?)))?)
    }

    /// Events of these turns, oldest first.
    pub fn events_for_prompts(&self, prompt_ids: &[String]) -> Result<Vec<EventRow>> {
        if prompt_ids.is_empty() {
            return Ok(Vec::new());
        }
        let turns = json_list(prompt_ids)?;
        Ok(self.db().read(|conn| events_of_turns(conn, turns))?)
    }

    /// Events newer than `event_id` whose name starts with `prefix` or is one of `names`, oldest first.
    pub fn events_after(&self, event_id: i64, prefix: &str, names: &[&str]) -> Result<Vec<EventRow>> {
        let names = json_list(names)?;
        let prefix_chars = int(prefix.chars().count());
        Ok(self.db().read(|conn| {
            let select =
                "select * from event where id > ? and (substr(event, 1, ?) = ? or event in (select value from json_each(?))) order by id";
            decode::all(conn, select, (event_id, prefix_chars, prefix, names), decode::event_row)
        })?)
    }

    /// The id of the newest stored event, 0 when there is none (a rowid lookup).
    pub fn newest_event_id(&self) -> Result<i64> {
        Ok(self.db().read(|conn| Ok(conn.prepare_cached("select coalesce(max(id), 0) from event")?.query_row([], |row| row.get(0))?))?)
    }

    /// Whether [`Self::events_after`] would return anything (reads no payload).
    pub fn has_events_after(&self, event_id: i64, prefix: &str, names: &[&str]) -> Result<bool> {
        let names = json_list(names)?;
        let prefix_chars = int(prefix.chars().count());
        Ok(self.db().read(|conn| {
            let select = "select exists (select 1 from event where id > ? and (substr(event, 1, ?) = ? or event in (select value from json_each(?))))";
            Ok(conn.prepare_cached(select)?.query_row((event_id, prefix_chars, prefix, names), |row| row.get(0))?)
        })?)
    }

    pub fn events_for_session(&self, session_id: &str) -> Result<Vec<EventRow>> {
        Ok(self
            .db()
            .read(|conn| decode::all(conn, "select * from event where session_id = ? order by id", [session_id], decode::event_row))?)
    }

    /// The most recent payloads (up to `limit` events and `limit` prompts, each prompt followed by its
    /// ingest context), for the field catalog.
    pub fn samples(&self, limit: usize) -> Result<Vec<Sample>> {
        let limit = int(limit);
        let named = |name: Option<String>| name.unwrap_or_else(|| "?".into());
        Ok(self.db().read(|conn| {
            let events = "select source, event, raw from event order by id desc limit ?";
            let mut samples = decode::all(conn, events, [limit], |row| {
                Ok(Sample { source: row.get("source")?, event: named(decode::text_at(row, "event")?), data: decode::json(row, "raw")? })
            })?;
            let prompts = "select source, json_extract(fields, '$.event') event, raw, context from prompt order by id desc limit ?";
            let pairs = decode::all(conn, prompts, [limit], |row| {
                let payload =
                    Sample { source: row.get("source")?, event: named(decode::text_at(row, "event")?), data: decode::json(row, "raw")? };
                let context =
                    Sample { source: "gitprompt".into(), event: "context (at ingest)".into(), data: decode::json(row, "context")? };
                Ok([payload, context])
            })?;
            samples.extend(pairs.into_iter().flatten());
            Ok(samples)
        })?)
    }

    /// Per source: prompt total, prompts with no text, event count, last activity (prompt or event).
    pub fn source_stats(&self) -> Result<BTreeMap<String, SourceStats>> {
        Ok(self.db().read(|conn| {
            let mut out = BTreeMap::new();
            let prompts =
                "select source, count(*) total, sum(prompt is null) unmapped, max(received_at) last_at from prompt group by source";
            for (source, stats) in decode::all(conn, prompts, [], |row| {
                let stats =
                    SourceStats { total: count(row, "total")?, unmapped: count(row, "unmapped")?, events: 0, last_at: row.get("last_at")? };
                Ok((row.get::<_, String>("source")?, stats))
            })? {
                out.insert(source, stats);
            }
            let events = "select source, sum(rows) events, max(last_at) last_at from event_stats group by source";
            for (source, events, last_at) in decode::all(conn, events, [], |row| {
                Ok((row.get::<_, String>("source")?, count(row, "events")?, row.get::<_, Option<String>>("last_at")?))
            })? {
                let stats: &mut SourceStats = out.entry(source).or_default();
                stats.events = events;
                stats.last_at = stats.last_at.take().max(last_at);
            }
            Ok(out)
        })?)
    }

    /// Rows (prompts and events) per source and UTC hour received at or after `since`. Cheap on a
    /// large table without an index on `received_at`: rows are inserted in time order, so the newest
    /// row older than `since` is found by walking ids backwards, and only newer ids are read.
    pub fn hourly(&self, since: &str) -> Result<Vec<HourCount>> {
        Ok(self.db().read(|conn| {
            let mut out = Vec::new();
            for table in ["prompt", "event"] {
                let older = format!("select id from {table} where received_at < ? order by id desc limit 1");
                let older: i64 = decode::first(conn, &older, [since], |row| Ok(row.get(0)?))?.unwrap_or(0);
                let counts = format!(
                    "select source, substr(received_at, 1, 13) hour, count(*) n from {table} where id > ? and received_at >= ? group by source, hour"
                );
                out.extend(decode::all(conn, &counts, (older, since), |row| {
                    Ok(HourCount { source: row.get("source")?, hour: row.get("hour")?, rows: count(row, "n")? })
                })?);
            }
            Ok(out)
        })?)
    }
}

/// `prompt_view` rows of these prompts, in the order given (unknown ids are skipped).
pub(super) fn views(conn: &Connection, ids: &[i64]) -> DbResult<Vec<PromptView>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let select = VIEW_OF_IDS;
    let mut by_id: HashMap<i64, PromptView> = decode::all(conn, select, [json_list(ids)?], |row| {
        let view = PromptView {
            row: decode::prompt_row(row)?,
            annotation: decode::annotation(&row.get::<_, String>("annotation")?),
            turn: row.get("turn")?,
            prev_at: decode::text_at(row, "prev_at")?,
            next_head: decode::text_at(row, "next_head")?,
        };
        Ok((view.row.id, view))
    })?
    .into_iter()
    .collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Events of the turns in the JSON array `turns`, oldest first.
pub(super) fn events_of_turns(conn: &Connection, turns: rusqlite::types::Value) -> DbResult<Vec<EventRow>> {
    let select = "select * from event where prompt_id in (select value from json_each(?)) order by id";
    decode::all(conn, select, [turns], decode::event_row)
}
