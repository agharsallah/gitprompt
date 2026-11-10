//! Edits: annotations, rewriting stored JSON (secret hiding), remapping a source.
//!
//! The caller's callbacks (`edit`, `mapper`) run outside any connection lock, inside the
//! transaction: they may even use the store themselves.

use gitprompt_core::{Error, Json, JsonMap, Result};

use super::Store;
use super::reads::events_of_turns;
use crate::decode::{self, field_text};
use crate::rows::Annotation;
use crate::sql::json_list;

impl Store {
    pub fn annotation(&self, id: i64) -> Result<Annotation> {
        let stored = self
            .db()
            .read(|conn| decode::first(conn, "select data from annotation where id = ?", [id], |row| Ok(row.get::<_, String>(0)?)))?;
        Ok(stored.map(|data| decode::annotation(&data)).unwrap_or_default())
    }

    pub fn annotate(&self, id: i64, data: &Annotation) -> Result<()> {
        let data = serde_json::to_string(data)?;
        Ok(self.db().write(|conn| {
            conn.prepare_cached("insert or replace into annotation (id, data) values (?, ?)")?.execute((id, &data))?;
            Ok(())
        })?)
    }

    /// Apply `edit` to a prompt's fields, context and raw, and to its turn's events' fields and raw
    /// (e.g. to hide a secret), in one transaction. Returns rows changed (0 for an unknown id).
    /// `edit` must keep fields and context JSON objects (else `Error::Invalid`, nothing changed).
    pub fn rewrite(&self, id: i64, edit: &dyn Fn(&Json) -> Json) -> Result<u64> {
        self.transaction(|store| {
            let Some(row) = store.prompts(&[id])?.pop() else {
                return Ok(0);
            };
            let events = match field_text(&row.fields, "prompt_id") {
                Some(turn) => store.db().read(|conn| events_of_turns(conn, json_list(&[turn])?))?,
                None => Vec::new(),
            };
            let fields = object(edit(&Json::Object(row.fields)), "fields")?;
            let prompt = fields.get("prompt").and_then(Json::as_str).map(str::to_owned);
            let stored = (
                serde_json::to_string(&fields)?,
                serde_json::to_string(&object(edit(&Json::Object(row.context)), "context")?)?,
                serde_json::to_string(&edit(&row.raw))?,
            );
            let edited_events = events
                .into_iter()
                .map(|e| {
                    let fields = serde_json::to_string(&object(edit(&Json::Object(e.fields)), "event fields")?)?;
                    Ok((e.id, fields, serde_json::to_string(&edit(&e.raw))?))
                })
                .collect::<Result<Vec<_>>>()?;
            store.db().write(|conn| {
                conn.prepare_cached("update prompt set prompt = ?, fields = ?, context = ?, raw = ? where id = ?")?
                    .execute((&prompt, &stored.0, &stored.1, &stored.2, id))?;
                let mut update = conn.prepare_cached("update event set fields = ?, raw = ? where id = ?")?;
                for (event_id, fields, raw) in &edited_events {
                    update.execute((fields, raw, event_id))?;
                }
                Ok(())
            })?;
            Ok(1 + edited_events.len() as u64)
        })
    }

    /// Re-run `mapper(raw) -> fields` over every stored payload of a source (after a spec fix);
    /// event columns follow the new fields. Returns rows remapped.
    pub fn remap(&self, source: &str, mapper: &dyn Fn(&Json) -> JsonMap) -> Result<u64> {
        self.transaction(|store| {
            let payloads = |table: &str| {
                let sql = format!("select id, raw from {table} where source = ? order by id");
                store.db().read(|conn| decode::all(conn, &sql, [source], |row| Ok((row.get::<_, i64>(0)?, decode::json(row, "raw")?))))
            };
            let prompts: Vec<(i64, JsonMap)> = payloads("prompt")?.into_iter().map(|(id, raw)| (id, mapper(&raw))).collect();
            let events: Vec<(i64, JsonMap)> = payloads("event")?.into_iter().map(|(id, raw)| (id, mapper(&raw))).collect();
            store.db().write(|conn| {
                let mut update = conn.prepare_cached("update prompt set prompt = ?, fields = ? where id = ?")?;
                for (id, fields) in &prompts {
                    let prompt = fields.get("prompt").and_then(Json::as_str);
                    update.execute((prompt, serde_json::to_string(fields)?, id))?;
                }
                let mut update =
                    conn.prepare_cached("update event set event = ?, session_id = ?, prompt_id = ?, fields = ? where id = ?")?;
                for (id, fields) in &events {
                    let columns = (field_text(fields, "event"), field_text(fields, "session_id"), field_text(fields, "prompt_id"));
                    update.execute((columns.0, columns.1, columns.2, serde_json::to_string(fields)?, id))?;
                }
                // Renamed events moved between summary groups; the triggers can't know an old
                // group's newest time, so this source's summary is recomputed.
                for statement in crate::schema::REBUILD_SOURCE_EVENT_STATS.split_terminator(';') {
                    conn.execute(statement, [source])?;
                }
                Ok(())
            })?;
            Ok((prompts.len() + events.len()) as u64)
        })
    }
}

fn object(value: Json, what: &str) -> Result<JsonMap> {
    match value {
        Json::Object(map) => Ok(map),
        other => Err(Error::Invalid(format!("an edit must keep the {what} a JSON object, got {other}"))),
    }
}
