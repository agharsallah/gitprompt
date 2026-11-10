//! From redacted input to a [`Report`] and its rows, one builder per kind of export (ADR-0018).
//! Pure: no clock, no I/O. A builder only ever sees what the options keep (the redaction pass ran
//! first); a text the options dropped is simply `None` here.
//!
//! Port of `export/build.py`. Number formats follow it: thousands separators (`1,234`), money with
//! two decimals (`$1,234.57`, `<$0.01` below a cent), em dash (`—`) for "not reported", U+2212 (`−`)
//! for deletions.
//!
//! ```text
//! read      reading values: labels, numbers, times, facts, charts, filters, notes
//! session   project   overview   prompts      one builder per kind
//! ```

mod overview;
mod project;
mod prompts;
mod read;
mod session;

use serde::Serialize;

use gitprompt_core::Json;

use crate::input::{Commit, ExportInput};
use crate::redact::Options;
use crate::report::{Block, Figure, Level, Report, Section};
use crate::{Kind, Row, Zone};

pub use overview::overview;
pub use project::project;
pub use prompts::prompts;
pub use read::{
    bars, changes, describe_filters, duration, fact_columns, integer, marker_label, model, money, notes, origin_label, outcome_chart,
    outcome_label, phase_reason, plural, reported, span, tokens, tool, when,
};
pub use session::session;

/// Items in a bar chart.
pub const TOP: usize = 10;
/// Characters of a stored payload shown in a document (the JSON and CSV rows hold all of it).
pub const RAW_CHARS: usize = 2000;
/// Shown for a value the tool did not report (never `0`).
pub const DASH: &str = "—";

/// Fact keys copied as row columns, after `tool` and `surface`.
pub const FACT_COLUMNS: &[&str] = &["model", "permission_mode", "tool_version", "api_requests"];
/// Columns of an Overview statistics row, after `grouping`, `key` (and `name`).
pub const BUCKET_COLUMNS: &[&str] = &[
    "prompts",
    "sessions",
    "cost_usd",
    "input_tokens",
    "output_tokens",
    "tool_calls",
    "tool_failures",
    "failed",
    "files_written",
    "git_commits",
    "subagents",
    "response_s_p50",
    "response_s_p90",
    "first_at",
    "last_at",
];
/// Columns of a prompt-selection row, before the fact columns.
pub const PROMPT_COLUMNS: &[&str] = &[
    "id",
    "received_at",
    "source",
    "session",
    "repo",
    "branch",
    "origin",
    "slash_command",
    "outcome",
    "chars",
    "cost_usd",
    "input_tokens",
    "output_tokens",
    "response_s",
    "tool_calls",
    "tool_failures",
    "subagents",
    "files_written",
    "git_commits",
];
/// The groupings of an Overview, in row order.
pub const OVERVIEW_GROUPS: &[&str] = &["day", "origin", "model", "tool_kind", "outcome", "repo", "command"];

/// The builder of the input's kind.
pub fn build(input: &ExportInput, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    match input {
        ExportInput::Session(data) => session(data, options, zone),
        ExportInput::Project(data) => project(data, options, zone),
        ExportInput::Overview(data) => overview(data, options, zone),
        ExportInput::Prompts(data) => prompts(data, options, zone),
    }
}

// --- shared by the builders ----------------------------------------------------------------------

/// The report's head and notes around its sections.
fn report(
    kind: Kind,
    generated_at: &str,
    title: String,
    subtitle: String,
    sections: Vec<Section>,
    options: &Options,
    zone: Zone,
) -> Report {
    Report { kind, title, subtitle, generated_at: when(Some(generated_at), false, zone), sections, notes: notes(options, kind) }
}

/// `value` as JSON. Every value given here is plain data (strings, numbers, options, maps with
/// string keys), which always serializes; a non-finite float becomes `null`, as serde_json writes it.
fn json(value: impl Serialize) -> Json {
    serde_json::to_value(value).unwrap_or(Json::Null)
}

/// `value`'s fields as a row (empty if it is not a struct or map).
fn fields(value: impl Serialize) -> Row {
    match json(value) {
        Json::Object(map) => map,
        _ => Row::new(),
    }
}

/// `columns` of `from`, in that order; a missing one is `null`.
fn pick(from: &Row, columns: &[&str]) -> Row {
    columns.iter().map(|c| (c.to_string(), from.get(*c).cloned().unwrap_or(Json::Null))).collect()
}

/// `extra` merged into `row`: a key it already has keeps its place and takes the new value.
fn merge(row: &mut Row, extra: Row) {
    for (key, value) in extra {
        row.insert(key, value);
    }
}

/// The text, or `None` when absent or empty (Python's `or` on a string).
fn present(text: Option<&str>) -> Option<&str> {
    text.filter(|t| !t.is_empty())
}

fn figure(label: &str, value: &str) -> Figure {
    Figure { label: label.to_string(), value: value.to_string() }
}

fn section(title: &str, blocks: Vec<Block>, subtitle: String, level: Level) -> Section {
    Section { title: title.to_string(), blocks, subtitle, level }
}

/// A commit as one line: `abc1234 Add app (+3 −0) — Jane Doe`.
fn commit_line(commit: &Commit) -> String {
    let hash: String = commit.hash.chars().take(7).collect();
    let author = present(commit.author.as_deref()).map(|a| format!(" — {a}")).unwrap_or_default();
    format!("{hash} {} (+{} −{}){author}", commit.subject, commit.insertions, commit.deletions)
}

/// Counts in a map, summed per key in first-seen order.
fn tally<'a>(into: &mut indexmap::IndexMap<String, i64>, counts: impl IntoIterator<Item = (&'a String, &'a i64)>) {
    for (key, n) in counts {
        *into.entry(key.clone()).or_default() += n;
    }
}

/// Tool calls per kind as read: `write 2, read 1`, most first (ties in order); [`DASH`] for none.
fn tools_line(tools: &indexmap::IndexMap<String, i64>) -> String {
    let mut sorted: Vec<(&String, &i64)> = tools.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    let line = sorted.iter().map(|(kind, n)| format!("{kind} {n}")).collect::<Vec<_>>().join(", ");
    if line.is_empty() { DASH.to_string() } else { line }
}

/// A JSON value Python would call true: not null, false, zero or empty.
fn truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Json::String(s) => !s.is_empty(),
        Json::Array(items) => !items.is_empty(),
        Json::Object(map) => !map.is_empty(),
    }
}
