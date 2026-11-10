//! Writes from the receiver, and the session join of tools that send no prompt id.

use rusqlite::Connection;

use gitprompt_core::{Json, JsonMap, Result};

use super::{NewEvent, Store};
use crate::decode::{self, field_text};
use crate::error::DbResult;
use crate::rows::{EventRow, PromptRow};

/// The session key of a prompt (no session = ''): the expression of the `prompt_session` index.
const SESSION: &str = "coalesce(json_extract(fields, '$.session_id'), '')";

impl Store {
    /// Insert a prompt; `None` if this source already sent the same `prompt_id` (dedup). Prompts
    /// without a `prompt_id` never collide. `prompt` is `fields.prompt` when it is a string.
    pub fn add_prompt(&self, source: &str, fields: &JsonMap, context: &JsonMap, raw: &Json) -> Result<Option<PromptRow>> {
        self.add_prompt_at(source, fields, context, raw, &self.now())
    }

    /// [`Self::add_prompt`] with an explicit `received_at` (UTC ISO-8601, stored as given) instead of
    /// now. New: imports, and tests that need prompts at known times.
    pub fn add_prompt_at(
        &self,
        source: &str,
        fields: &JsonMap,
        context: &JsonMap,
        raw: &Json,
        received_at: &str,
    ) -> Result<Option<PromptRow>> {
        let prompt = fields.get("prompt").and_then(Json::as_str).map(str::to_owned);
        let stored = (serde_json::to_string(fields)?, serde_json::to_string(context)?, serde_json::to_string(raw)?);
        let id = self.db().write(|conn| {
            let insert = "insert into prompt (source, received_at, prompt, fields, context, raw) values (?, ?, ?, ?, ?, ?) \
                 on conflict do nothing returning id";
            decode::first(conn, insert, (source, received_at, &prompt, &stored.0, &stored.1, &stored.2), |row| Ok(row.get(0)?))
        })?;
        Ok(id.map(|id| PromptRow {
            id,
            source: source.to_owned(),
            received_at: received_at.to_owned(),
            prompt,
            fields: fields.clone(),
            context: context.clone(),
            raw: raw.clone(),
        }))
    }

    /// Insert an event; `event`, `session_id`, `prompt_id` columns come from `fields`.
    pub fn add_event(&self, source: &str, fields: &JsonMap, raw: &Json) -> Result<EventRow> {
        self.add_event_at(source, fields, raw, &self.now())
    }

    /// [`Self::add_event`] with an explicit `received_at` (UTC ISO-8601, stored as given). New.
    pub fn add_event_at(&self, source: &str, fields: &JsonMap, raw: &Json, received_at: &str) -> Result<EventRow> {
        Ok(self.db().write(|conn| insert_event(conn, source, fields, raw, received_at))?)
    }

    /// Insert many events in one transaction (an OTLP batch): one fsync instead of one per row.
    pub fn add_events(&self, events: &[NewEvent]) -> Result<Vec<EventRow>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let received_at = self.now();
        Ok(self.db().atomic(|conn| events.iter().map(|e| insert_event(conn, &e.source, &e.fields, &e.raw, &received_at)).collect())?)
    }

    // --- joining events to prompts without a prompt id (spec `join: session`) ---

    /// Store `turn_name` as the prompt's `fields.prompt_id`; returns it.
    pub fn name_turn(&self, id: i64, source: &str) -> Result<String> {
        let turn = Self::turn_name(source, id);
        self.db().write(|conn| {
            conn.prepare_cached("update prompt set fields = json_set(fields, '$.prompt_id', ?) where id = ?")?.execute((&turn, id))?;
            Ok(())
        })?;
        Ok(turn)
    }

    /// The prompt id of the latest prompt of this source and session.
    pub fn latest_turn(&self, source: &str, session_id: &str) -> Result<Option<String>> {
        let sql =
            format!("select json_extract(fields, '$.prompt_id') from prompt where source = ? and {SESSION} = ? order by id desc limit 1");
        let turn = self.db().read(|conn| decode::first(conn, &sql, (source, session_id), |row| Ok(decode::text(row.get_ref(0)?))))?;
        Ok(turn.flatten())
    }

    /// Redo the session join over everything stored for a source (after a remap): name prompts that
    /// have no prompt id, attach each event without one to the latest prompt of its session received
    /// at or before it, and copy that id into the event's fields.
    pub fn rejoin(&self, source: &str) -> Result<()> {
        Ok(self.db().atomic(|conn| {
            conn.execute(
                "update prompt set fields = json_set(fields, '$.prompt_id', source || '#' || id) \
                 where source = ? and json_extract(fields, '$.prompt_id') is null",
                [source],
            )?;
            conn.execute(
                "update event set prompt_id = (
                   select json_extract(p.fields, '$.prompt_id') from prompt p
                   where p.source = event.source and coalesce(json_extract(p.fields, '$.session_id'), '') = event.session_id
                     and p.received_at <= event.received_at
                   order by p.id desc limit 1)
                 where source = ? and session_id is not null and prompt_id is null",
                [source],
            )?;
            conn.execute(
                "update event set fields = json_set(fields, '$.prompt_id', prompt_id) \
                 where source = ? and prompt_id is not null and json_extract(fields, '$.prompt_id') is null",
                [source],
            )?;
            Ok(())
        })?)
    }
}

fn insert_event(conn: &Connection, source: &str, fields: &JsonMap, raw: &Json, received_at: &str) -> DbResult<EventRow> {
    let (event, session_id, prompt_id) = (field_text(fields, "event"), field_text(fields, "session_id"), field_text(fields, "prompt_id"));
    let mut insert = conn.prepare_cached(
        "insert into event (source, received_at, event, session_id, prompt_id, fields, raw) values (?, ?, ?, ?, ?, ?, ?) returning id",
    )?;
    let id: i64 = insert.query_row(
        (source, received_at, &event, &session_id, &prompt_id, serde_json::to_string(fields)?, serde_json::to_string(raw)?),
        |row| row.get(0),
    )?;
    Ok(EventRow {
        id,
        source: source.to_owned(),
        received_at: received_at.to_owned(),
        event,
        session_id,
        prompt_id,
        fields: fields.clone(),
        raw: raw.clone(),
    })
}
