//! Query-string parsing for the prompt filters and the routes that take them (port of
//! `api/filters.py`). Repeated keys build lists (`origin=typed&origin=slash_command`), as the
//! dashboard sends them; empty values are ignored; unknown keys are ignored.
//!
//! * `since` / `until`: ISO-8601; without an offset they are local time (the process's time zone).
//!   Normalized to UTC ISO-8601 with microseconds, as the store keeps instants.
//! * `weekday` 1..=7, `hour` 0..=23 (0 is a value), `limit` per route, `starred=true|false`.
//! * An invalid value of any known key (unknown origin, outcome, `has`, `group_by`...) is
//!   [`ApiError::Unprocessable`] naming the key.

use jiff::Timestamp;
use jiff::civil;
use jiff::tz::TimeZone;
use serde::de::DeserializeOwned;

use gitprompt_store::filter::{Outcome, PromptFilter};

use crate::error::ApiError;
use crate::model::{ExportQuery, PromptQuery, StatsQuery};

/// `(key, value)` pairs of a raw query string, percent-decoded, in order.
pub fn pairs(query: Option<&str>) -> Vec<(String, String)> {
    query
        .unwrap_or_default()
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (decode(key), decode(value))
        })
        .collect()
}

/// `+` as a space, `%XX` as its byte; malformed escapes kept as they are; invalid UTF-8 replaced.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'+' => out.push(b' '),
            b'%' => match bytes.get(i + 1..i + 3).and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    i += 2;
                }
                None => out.push(b'%'),
            },
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The non-empty values of the pairs, in order.
fn values(pairs: &[(String, String)]) -> impl Iterator<Item = (&str, &str)> {
    pairs.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k.as_str(), v.as_str()))
}

fn invalid(key: &str, value: &str, why: &str) -> ApiError {
    ApiError::Unprocessable(format!("{key}: {why}, got {value:?}"))
}

/// A unit enum by its wire name (its serde name).
fn variant<T: DeserializeOwned>(key: &str, value: &str) -> Result<T, ApiError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|_| invalid(key, value, "not a known value"))
}

fn integer<T: std::str::FromStr + PartialOrd + Copy>(key: &str, value: &str, range: std::ops::RangeInclusive<T>) -> Result<T, ApiError> {
    match value.trim().parse::<T>() {
        Ok(n) if range.contains(&n) => Ok(n),
        Ok(_) => Err(invalid(key, value, "out of range")),
        Err(_) => Err(invalid(key, value, "not a whole number")),
    }
}

/// A boolean as FastAPI reads one: true/false, 1/0, yes/no, on/off (any case).
pub(crate) fn boolean(key: &str, value: &str) -> Result<bool, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "t" | "y" => Ok(true),
        "false" | "0" | "no" | "off" | "f" | "n" => Ok(false),
        _ => Err(invalid(key, value, "not a boolean")),
    }
}

/// An instant as the store keeps it (UTC ISO-8601, microseconds, `+00:00`). With an offset (or
/// `Z`) as given; without one (a date and time, or a date at midnight) in the process's local time.
pub fn instant(key: &str, value: &str) -> Result<String, ApiError> {
    let text = value.trim();
    let at = text
        .parse::<Timestamp>()
        .ok()
        .or_else(|| local(text.parse::<civil::DateTime>().ok()?))
        .or_else(|| local(text.parse::<civil::Date>().ok()?.to_datetime(civil::Time::midnight())))
        .ok_or_else(|| invalid(key, value, "not an ISO-8601 date and time"))?;
    Ok(iso(at))
}

fn local(naive: civil::DateTime) -> Option<Timestamp> {
    // An ambiguous local time (DST fold) takes the earlier instant, a skipped one the later.
    TimeZone::system().to_ambiguous_zoned(naive).compatible().ok().map(|z| z.timestamp())
}

/// `received_at` format: UTC, microseconds, `+00:00`.
pub(crate) fn iso(at: Timestamp) -> String {
    at.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
}

/// The prompt filters among the pairs.
pub fn prompt_filter(pairs: &[(String, String)]) -> Result<PromptFilter, ApiError> {
    let mut f = PromptFilter::default();
    for (key, value) in values(pairs) {
        let text = || Some(value.to_owned());
        match key {
            "q" => f.q = text(),
            "origin" => f.origin.push(variant(key, value)?),
            "model" => f.model.push(value.to_owned()),
            "surface" => f.surface.push(value.to_owned()),
            "session" => f.session.push(value.to_owned()),
            "source" => f.source = text(),
            "repo" => f.repo = text(),
            "since" => f.since = Some(instant(key, value)?),
            "until" => f.until = Some(instant(key, value)?),
            "weekday" => f.weekday = Some(integer(key, value, 1..=7)?),
            "hour" => f.hour = Some(integer(key, value, 0..=23)?),
            "command" => f.command = text(),
            "tool" => f.tool = text(),
            "tool_kind" => f.tool_kind = Some(variant(key, value)?),
            "query_source" => f.query_source = text(),
            "outcome" => f.outcome.push(Outcome::from_name(value).ok_or_else(|| invalid(key, value, "not a known outcome"))?),
            "size" => f.size = text(),
            "has" => f.has.push(variant(key, value)?),
            "tag" => f.tag = text(),
            "starred" => f.starred = Some(boolean(key, value)?),
            "before" => f.before = Some(integer(key, value, i64::MIN..=i64::MAX)?),
            _ => {}
        }
    }
    Ok(f)
}

/// The last value of `key` that is not empty.
fn last<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    values(pairs).filter(|(k, _)| *k == key).map(|(_, v)| v).last()
}

/// Filters plus `limit` (1..=200, default 50): `/api/prompts`, `/api/search`.
pub fn prompt_query(pairs: &[(String, String)]) -> Result<PromptQuery, ApiError> {
    let limit = last(pairs, "limit").map(|v| integer("limit", v, 1..=200)).transpose()?.unwrap_or(50);
    Ok(PromptQuery { filter: prompt_filter(pairs)?, limit })
}

/// Filters plus `group_by`, `split_by` (needs `group_by`), `order`, `limit` (1..=10000).
pub fn stats_query(pairs: &[(String, String)]) -> Result<StatsQuery, ApiError> {
    let group_by = last(pairs, "group_by").map(|v| variant("group_by", v)).transpose()?;
    let split_by = last(pairs, "split_by").map(|v| variant("split_by", v)).transpose()?;
    if split_by.is_some() && group_by.is_none() {
        return Err(ApiError::Unprocessable("split_by needs group_by".into()));
    }
    let order = last(pairs, "order").map(|v| variant("order", v)).transpose()?;
    let limit = last(pairs, "limit").map(|v| integer("limit", v, 1..=10_000)).transpose()?.unwrap_or(1000);
    Ok(StatsQuery { filter: prompt_filter(pairs)?, group_by, split_by, order, limit })
}

/// The export options among the pairs (the filters are read separately).
pub fn export_query(pairs: &[(String, String)]) -> Result<ExportQuery, ApiError> {
    let mut q = ExportQuery::default();
    for (key, value) in values(pairs) {
        match key {
            "id" => q.id = Some(value.to_owned()),
            "format" => q.format = variant(key, value)?,
            "chart" => q.chart = Some(value.to_owned()),
            "download" => q.download = boolean(key, value)?,
            "prompt_text" => q.prompt_text = boolean(key, value)?,
            "relative_paths" => q.relative_paths = boolean(key, value)?,
            "drop_identity" => q.drop_identity = boolean(key, value)?,
            "drop_raw" => q.drop_raw = boolean(key, value)?,
            _ => {}
        }
    }
    Ok(q)
}

/// `dry_run` of the write routes, with the route's default.
pub fn dry_run(pairs: &[(String, String)], default: bool) -> Result<bool, ApiError> {
    last(pairs, "dry_run").map(|v| boolean("dry_run", v)).transpose().map(|v| v.unwrap_or(default))
}
