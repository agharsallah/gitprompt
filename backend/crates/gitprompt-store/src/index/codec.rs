//! Index rows to and from SQL: the `prompt_index` columns in one place, so inserts and reads
//! can't drift apart.

use rusqlite::Row;
use rusqlite::types::Value;

use gitprompt_core::spec::ToolKind;

use super::{IndexRow, ToolCalls, UsageTotals};
use crate::decode::text_at;
use crate::error::{DbResult, failure};
use crate::sql::filter::name;

/// The usage columns of the per-model and per-work tables, in [`UsageTotals`] order.
pub(super) const USAGE: &str = "api_requests, cost_usd, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens";

/// The `prompt_index` columns, in the order [`values`] gives them. The typed text is only in
/// `prompt_fts`: keeping it out keeps index rows small, so aggregates scan fewer pages.
#[rustfmt::skip]
pub(super) const COLUMNS: [&str; 46] = [
    "id", "prompt_id", "source", "session", "repo", "repo_name", "branch", "title",
    "received_at", "day", "hour", "weekday",
    "origin", "chars", "text_hash", "slash_command", "code_blocks", "urls", "mentions", "secrets",
    "api_requests", "cost_usd", "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens",
    "api_time_s", "api_errors", "response_s", "ended_by",
    "tool_calls", "tool_failures", "tool_time_s", "subagent_tool_calls", "subagents",
    "files_written", "git_commits", "compactions", "denials", "rejections",
    "model", "permission_mode", "tool_version", "reasoning_tokens", "answered", "surface",
];

/// A row's values in [`COLUMNS`] order.
#[rustfmt::skip]
pub(super) fn values(r: &IndexRow) -> [Value; 46] {
    let text = |v: &Option<String>| v.clone().map_or(Value::Null, Value::Text);
    let u = &r.usage;
    [
        r.id.into(), text(&r.prompt_id), r.source.clone().into(), text(&r.session), text(&r.repo), text(&r.repo_name), text(&r.branch), text(&r.title),
        r.received_at.clone().into(), r.day.clone().into(), i64::from(r.hour).into(), i64::from(r.weekday).into(),
        r.origin.clone().into(), r.chars.into(), text(&r.text_hash), text(&r.slash_command), r.code_blocks.into(), r.urls.into(), r.mentions.into(), r.secrets.into(),
        u.api_requests.into(), u.cost_usd.into(), u.input_tokens.into(), u.output_tokens.into(), u.cache_read_tokens.into(), u.cache_creation_tokens.into(),
        r.api_time_s.into(), r.api_errors.into(), r.response_s.map_or(Value::Null, Value::Real), text(&r.ended_by),
        r.tool_calls.into(), r.tool_failures.into(), r.tool_time_s.into(), r.subagent_tool_calls.into(), r.subagents.into(),
        r.files_written.into(), r.git_commits.into(), r.compactions.into(), r.denials.into(), r.rejections.into(),
        text(&r.model), text(&r.permission_mode), text(&r.tool_version), r.reasoning_tokens.into(), i64::from(r.answered).into(), text(&r.surface),
    ]
}

/// The select list of an [`IndexRow`] of `prompt_index i`, typed text included.
pub(crate) fn row_columns() -> String {
    let columns: Vec<String> = COLUMNS.iter().map(|c| format!("i.{c}")).collect();
    format!("{}, (select typed_text from prompt_fts where rowid = i.id) typed_text", columns.join(", "))
}

/// An [`IndexRow`] selected with [`row_columns`].
pub(crate) fn index_row(r: &Row<'_>) -> DbResult<IndexRow> {
    Ok(IndexRow {
        id: r.get("id")?,
        prompt_id: text_at(r, "prompt_id")?,
        source: r.get("source")?,
        session: text_at(r, "session")?,
        repo: text_at(r, "repo")?,
        repo_name: text_at(r, "repo_name")?,
        branch: text_at(r, "branch")?,
        title: text_at(r, "title")?,
        received_at: r.get("received_at")?,
        day: r.get("day")?,
        hour: small(r, "hour")?,
        weekday: small(r, "weekday")?,
        origin: r.get("origin")?,
        chars: r.get("chars")?,
        text_hash: text_at(r, "text_hash")?,
        slash_command: text_at(r, "slash_command")?,
        code_blocks: r.get("code_blocks")?,
        urls: r.get("urls")?,
        mentions: r.get("mentions")?,
        secrets: r.get("secrets")?,
        usage: usage(r)?,
        api_time_s: r.get("api_time_s")?,
        api_errors: r.get("api_errors")?,
        response_s: r.get("response_s")?,
        ended_by: text_at(r, "ended_by")?,
        tool_calls: r.get("tool_calls")?,
        tool_failures: r.get("tool_failures")?,
        tool_time_s: r.get("tool_time_s")?,
        subagent_tool_calls: r.get("subagent_tool_calls")?,
        subagents: r.get("subagents")?,
        files_written: r.get("files_written")?,
        git_commits: r.get("git_commits")?,
        compactions: r.get("compactions")?,
        denials: r.get("denials")?,
        rejections: r.get("rejections")?,
        model: text_at(r, "model")?,
        permission_mode: text_at(r, "permission_mode")?,
        tool_version: text_at(r, "tool_version")?,
        reasoning_tokens: r.get("reasoning_tokens")?,
        answered: r.get::<_, i64>("answered")? != 0,
        surface: text_at(r, "surface")?,
        typed_text: text_at(r, "typed_text")?,
    })
}

/// A tool kind as stored; one this version doesn't know counts as `other`.
pub(crate) fn tool_kind(stored: &str) -> ToolKind {
    serde_json::from_value(serde_json::Value::String(stored.to_owned())).unwrap_or_else(|_| {
        tracing::debug!(kind = stored, "unknown tool kind in the index");
        ToolKind::Other
    })
}

pub(super) fn tool_kinds(tools: &[ToolCalls]) -> DbResult<Vec<String>> {
    tools.iter().map(|t| name(&t.kind)).collect()
}

pub(super) fn usage(r: &Row<'_>) -> DbResult<UsageTotals> {
    Ok(UsageTotals {
        api_requests: r.get("api_requests")?,
        cost_usd: r.get("cost_usd")?,
        input_tokens: r.get("input_tokens")?,
        output_tokens: r.get("output_tokens")?,
        cache_read_tokens: r.get("cache_read_tokens")?,
        cache_creation_tokens: r.get("cache_creation_tokens")?,
    })
}

/// An hour or weekday column.
fn small(r: &Row<'_>, column: &str) -> DbResult<u8> {
    let value: i64 = r.get(column)?;
    u8::try_from(value).map_err(|_| failure(format!("stored {column} out of range: {value}")))
}
