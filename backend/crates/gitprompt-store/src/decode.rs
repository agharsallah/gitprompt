//! Reading stored rows back. Lenient where the Python daemon may have stored another type (a number
//! in a text column), strict where the store's own invariants hold (JSON columns are JSON).

use rusqlite::types::ValueRef;
use rusqlite::{Connection, Params, Row};

use gitprompt_core::{Json, JsonMap};

use crate::error::{DbResult, failure};
use crate::rows::{Annotation, EventRow, PromptRow};

/// Every row of a query (prepared through the connection's statement cache), decoded by `decode`.
pub(crate) fn all<T>(
    conn: &Connection,
    sql: &str,
    params: impl Params,
    mut decode: impl FnMut(&Row<'_>) -> DbResult<T>,
) -> DbResult<Vec<T>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let mut rows = stmt.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(decode(row)?);
    }
    Ok(out)
}

/// The first row of a query, if any.
pub(crate) fn first<T>(
    conn: &Connection,
    sql: &str,
    params: impl Params,
    decode: impl FnOnce(&Row<'_>) -> DbResult<T>,
) -> DbResult<Option<T>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let mut rows = stmt.query(params)?;
    rows.next()?.map(decode).transpose()
}

/// A text value; numbers (which Python may have stored in a text column) as their decimal text.
pub(crate) fn text(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Null | ValueRef::Blob(_) => None,
    }
}

pub(crate) fn text_at(row: &Row<'_>, column: &str) -> DbResult<Option<String>> {
    Ok(text(row.get_ref(column)?))
}

/// A JSON value stored as text.
pub(crate) fn json(row: &Row<'_>, column: &str) -> DbResult<Json> {
    let stored: String = row.get(column)?;
    Ok(serde_json::from_str(&stored)?)
}

/// A JSON object stored as text.
pub(crate) fn json_object(row: &Row<'_>, column: &str) -> DbResult<JsonMap> {
    match json(row, column)? {
        Json::Object(map) => Ok(map),
        other => Err(failure(format!("stored {column} is not a JSON object: {other}"))),
    }
}

pub(crate) fn prompt_row(row: &Row<'_>) -> DbResult<PromptRow> {
    Ok(PromptRow {
        id: row.get("id")?,
        source: row.get("source")?,
        received_at: row.get("received_at")?,
        prompt: text_at(row, "prompt")?,
        fields: json_object(row, "fields")?,
        context: json_object(row, "context")?,
        raw: json(row, "raw")?,
    })
}

pub(crate) fn event_row(row: &Row<'_>) -> DbResult<EventRow> {
    Ok(EventRow {
        id: row.get("id")?,
        source: row.get("source")?,
        received_at: row.get("received_at")?,
        event: text_at(row, "event")?,
        session_id: text_at(row, "session_id")?,
        prompt_id: text_at(row, "prompt_id")?,
        fields: json_object(row, "fields")?,
        raw: json(row, "raw")?,
    })
}

/// An annotation as stored. One the dashboard can't read (written by hand, or by an older version)
/// shows as no annotation rather than failing every read of its prompt.
pub(crate) fn annotation(stored: &str) -> Annotation {
    serde_json::from_str(stored).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "ignoring an unreadable annotation");
        Annotation::default()
    })
}

/// A mapped field as an event column (`event`, `session_id`, `prompt_id`): text, numbers as their
/// decimal text, anything else none.
pub(crate) fn field_text(fields: &JsonMap, key: &str) -> Option<String> {
    match fields.get(key)? {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A count SQLite returns (never negative).
pub(crate) fn count(row: &Row<'_>, column: &str) -> DbResult<u64> {
    let n: Option<i64> = row.get(column)?;
    Ok(n.map_or(0, |n| u64::try_from(n).unwrap_or(0)))
}
