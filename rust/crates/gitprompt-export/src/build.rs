//! From redacted input to a [`Report`] and its rows, one builder per kind of export (ADR-0018).
//! Pure: no clock, no I/O. A builder only ever sees what the options keep (the redaction pass ran
//! first); a text the options dropped is simply `None` here.
//!
//! Port of `export/build.py`. Number formats follow it: thousands separators (`1,234`), money with
//! two decimals (`$1,234.57`, `<$0.01` below a cent), em dash (`—`) for "not reported", U+2212 (`−`)
//! for deletions.

use gitprompt_store::PromptFilter;

use crate::input::{Diffstat, ExportInput, Facts, OverviewExport, Phase, ProjectExport, PromptsExport, SessionExport};
use crate::redact::Options;
use crate::report::{Chart, Report};
use crate::{Kind, Row, Zone};

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

// --- reading values ------------------------------------------------------------------------------

/// How a turn ended, as read: `done` → `Done`, `open` → `No end seen`; an unknown key as is.
pub fn outcome_label(_key: &str) -> String {
    todo!()
}

/// Who wrote a prompt, as read: `slash_command` → `Slash command`; an unknown key as is.
pub fn origin_label(_key: &str) -> String {
    todo!()
}

/// A story marker's kind, as read: the origin labels plus `subagent_prompt` → `Subagent prompt` and
/// `compaction` → `Compaction`; an unknown key as is.
pub fn marker_label(_key: &str) -> String {
    todo!()
}

/// A count, rounded, with thousands separators; `None` is `0`.
pub fn integer(_n: Option<f64>) -> String {
    todo!()
}

/// `$1,234.57`; `<$0.01` for a cost under a cent but not zero; `None` and zero are `$0.00`.
pub fn money(_usd: Option<f64>) -> String {
    todo!()
}

/// A cost the tool reported, or [`DASH`] when it reported none (not `$0.00`).
pub fn reported(_usd: Option<f64>) -> String {
    todo!()
}

/// The tool that sent a prompt, with the app it was typed in when known:
/// `Claude Code (Desktop app)`; [`DASH`] without a connector label.
pub fn tool(_facts: Option<&Facts>) -> String {
    todo!()
}

/// The turn's model, or [`DASH`].
pub fn model(_facts: Option<&Facts>) -> String {
    todo!()
}

/// Tokens the tool reported (their total, as an [`integer`]), or [`DASH`] when it reported none.
pub fn tokens(_facts: Option<&Facts>) -> String {
    todo!()
}

/// Facts kept as row columns: `tool` (connector id), `surface` (its id, null when unknown), the
/// [`FACT_COLUMNS`], `tokens` (total), `cost_usd` (null when not reported) and `reasoning_tokens`.
/// Empty without facts.
pub fn fact_columns(_facts: Option<&Facts>) -> Row {
    todo!()
}

/// A duration as read: `42 s`, `3 min` (whole minutes, rounded down), `2 h 5 min`, `12 h` (minutes
/// shown under 10 h only), `1 h` (no `0 min`); [`DASH`] for `None`.
pub fn duration(_seconds: Option<f64>) -> String {
    todo!()
}

/// An instant in the reader's time zone: `2026-03-02 09:00`, or `2026-03-02` when `date_only`. A
/// value without an offset (a day key, a naive time) is already local. [`DASH`] for none or empty;
/// a value that isn't ISO-8601 is shown as is.
pub fn when(_iso: Option<&str>, _date_only: bool, _zone: Zone) -> String {
    todo!()
}

/// A time span as read: `2026-03-02 09:00 – 09:03` within a day, both dates otherwise, one time
/// when both read the same or there's no end, `""` without a start.
pub fn span(_first: Option<&str>, _last: Option<&str>, _zone: Zone) -> String {
    todo!()
}

/// `1 commit`, `2 commits`, `1,200 files`; `many` replaces the `s` plural.
pub fn plural(_n: i64, _word: &str, _many: Option<&str>) -> String {
    todo!()
}

/// `1 commit, +3 −0 in 1 file`, or `No commits`.
pub fn changes(_diffstat: Option<&Diffstat>) -> String {
    todo!()
}

/// A bar chart of the non-zero counts, largest first (ties keep their order), at most [`TOP`].
pub fn bars(_id: &str, _title: &str, _counts: Vec<(String, f64)>, _show: &dyn Fn(f64) -> String, _caption: &str) -> Chart {
    todo!()
}

/// Chart `outcomes`, "How the turns ended", labelled with [`outcome_label`].
pub fn outcome_chart(_outcomes: &indexmap::IndexMap<String, i64>) -> Chart {
    todo!()
}

/// The filters an Overview or a prompt selection was made with, as read, joined by ` · `:
/// `2026-03-01 to now · Typed, slash command · project shop · outcome done, no end seen ·
/// with tool failure · starred · matching “fix”`; `All time` without dates.
pub fn describe_filters(_filters: &PromptFilter, _zone: Zone) -> String {
    todo!()
}

/// What the export includes and leaves out: printed in the file itself, so its reader knows too.
/// The stored-payload note is only part of a `prompts` export.
pub fn notes(_options: &Options, _kind: Kind) -> Vec<String> {
    todo!()
}

/// Why a project phase starts: `After a pause of 3 h`, `On branch feat`, `Branch changed`, `Start`.
pub fn phase_reason(_phase: &Phase) -> String {
    todo!()
}

// --- builders ------------------------------------------------------------------------------------

/// A session: summary (figures, outcomes, tool kinds), the telemetry environment, then one level-3
/// section per chapter and per group of markers. Rows: one per chapter and per marker.
pub fn session(_data: &SessionExport, _options: &Options, _zone: Zone) -> (Report, Vec<Row>) {
    todo!()
}

/// A project: summary (measures, prompts per phase, outcomes, a note on other origins), then one
/// section per phase with its table of prompts. Rows: one per prompt, with its phase.
pub fn project(_data: &ProjectExport, _options: &Options, _zone: Zone) -> (Report, Vec<Row>) {
    todo!()
}

/// The Overview: summary and activity, prompt types, models and cost, agentic work, projects. Rows:
/// long-format statistics, a `total` row then one per bucket of each of [`OVERVIEW_GROUPS`].
pub fn overview(_data: &OverviewExport, _options: &Options, _zone: Zone) -> (Report, Vec<Row>) {
    todo!()
}

/// A prompt selection: summary, a table of prompts, and their stored payloads when collected
/// (each clipped to [`RAW_CHARS`] characters in the document). Rows: one per prompt.
pub fn prompts(_data: &PromptsExport, _options: &Options, _zone: Zone) -> (Report, Vec<Row>) {
    todo!()
}

/// The builder of the input's kind.
pub fn build(_input: &ExportInput, _options: &Options, _zone: Zone) -> (Report, Vec<Row>) {
    todo!()
}
