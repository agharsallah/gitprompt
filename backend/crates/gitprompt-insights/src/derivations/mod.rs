//! Metadata GitPrompt derives from what it stored. Computed on read, so a better rule applies to old
//! prompts too, and nothing here needs new access: it only reads stored rows.
//!
//! | value              | from                       | shown as                    |
//! |--------------------|----------------------------|-----------------------------|
//! | [`PromptStats`]    | the prompt text            | `PromptOut.stats`           |
//! | [`TurnSummary`]    | the prompt + its events    | `PromptOut.summary`         |
//! | [`SessionSummary`] | every event of the session | `PromptDetail.session`      |
//! | [`Derived`]        | all of the above + origin + surface | the prompt API, the index |
//!
//! Event names are Claude Code's; tools that copy its hook shape get these for free. What each tool
//! does comes from the source spec (`tool_kinds`, `tool_files`, `usage`, `answer`), and tool inputs
//! are read by generic names.
//!
//! Serialization matches the Python API: the summaries serialize *compacted* (null, `""`, `[]`,
//! `{}`, `false` and `0` values left out, `gitprompt_core::json::compact`), and a `None` map key
//! (a tool call without a tool name, a model call without a model) serializes as the key `"null"`.

mod serial;
mod session;
mod stats;
mod turn;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize, Serializer};

use gitprompt_core::spec::{Origin, Pattern, Spec, ToolKind, Usage};
use gitprompt_core::{Json, JsonMap};
use gitprompt_store::index::UsageTotals;
use gitprompt_store::{EventRow, PromptView};

use crate::origin::Classified;
use crate::raw::{self, Raw, Reads};

/// Events that end a turn. `Interrupt`: you stopped the turn (Codex, Cursor's aborted stop).
pub const TURN_END: &[&str] = &["Stop", "StopFailure", "Interrupt"];
/// Events that report a finished tool call.
pub const TOOL_RESULTS: &[&str] = &["PostToolUse", "PostToolUseFailure"];
/// Tool inputs naming the file a tool touched, by priority.
pub const FILE_INPUTS: &[&str] = &["file_path", "notebook_path", "path"];
/// What an agent tool was asked: its task's name (Claude Code's Agent: `description`; Codex's
/// `spawn_agent`: `task_name`)...
pub const TASK_INPUTS: &[&str] = &["description", "task_name"];
/// ...and its brief (`prompt`, `message`).
pub const BRIEF_INPUTS: &[&str] = &["prompt", "message"];
/// What went wrong, by the names the tools use (Cursor: `error_message`).
pub const ERROR_KEYS: &[&str] = &["error", "error_message"];
/// An error the tool reported during the turn (Copilot CLI).
pub const ERROR_EVENTS: &[&str] = &["ErrorOccurred"];
/// Characters of an error or a brief kept in a summary; longer texts are cut (trailing whitespace
/// removed) and end with `…`.
pub const TEXT_CHARS: usize = 280;
/// A shell command that commits: `git commit`, `git -C dir commit`, `git -c k=v commit`.
pub const GIT_COMMIT: &str = r"\bgit\b(?:\s+-\S+(?:\s+\S+)?)*\s+commit\b";

/// Telemetry metric event names start with this.
pub const METRIC_PREFIX: &str = "claude_code.";
/// OpenTelemetry resource attribute prefixes: the session's environment.
pub const ENVIRONMENT: &[&str] = &["app.", "terminal.", "os.", "host.", "service.", "vcs.", "user.", "organization."];
/// Slash commands in telemetry: a `user_prompt` event with a `command_name`.
pub const COMMAND_EVENT: &str = "user_prompt";
/// `query_source` values of model calls made for the main thread (plus no value at all, and
/// `repl_main_thread:<qualifier>`, e.g. with a custom output style).
pub const MAIN_QUERY_SOURCES: &[&str] = &["repl_main_thread", "sdk"];
/// `query_source` of a model call made by a subagent, e.g. `agent:builtin:Explore`.
pub const SUBAGENT_PREFIX: &str = "agent:";

// --- prompt text -----------------------------------------------------------------------------

/// What the prompt text itself says.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct PromptStats {
    /// Characters (not bytes).
    pub chars: usize,
    /// Whitespace-separated words.
    pub words: usize,
    /// Newlines + 1; 0 for an empty text.
    pub lines: usize,
    /// ⌈chars / 4⌉.
    pub approx_tokens: usize,
    /// The command the text starts with (after optional whitespace): `/` then word characters,
    /// `:` or `-`, e.g. `/review`, `/plugin:run`.
    pub slash_command: Option<String>,
    /// `@path` mentions preceded by whitespace or the start of the text, without the `@`
    /// (word characters, `.`, `/`, `~`, `-`).
    pub at_mentions: Vec<String>,
    /// `http(s)://...` up to whitespace, `)`, `>` or `]`.
    pub urls: Vec<String>,
    /// Fenced blocks: the number of triple-backtick fences divided by 2, rounded down.
    pub code_blocks: usize,
    /// Number of `?`.
    pub questions: usize,
    /// Kinds of credentials found (`gitprompt_core::secrets::find_secrets`), never the values.
    pub secrets: Vec<String>,
}

/// Compacted (zeros and empty lists left out), as `PromptOut.stats`.
impl Serialize for PromptStats {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serial::prompt_stats(self).serialize(s)
    }
}

pub fn prompt_stats(text: &str) -> PromptStats {
    stats::prompt_stats(text)
}

// --- one turn: the prompt and the events that share its prompt_id ---------------------------

/// Files one tool call touched: its path input (the first of [`FILE_INPUTS`] with a value), else
/// what the spec's `tool_files` pattern for the tool (by name, `prefix*` allowed) finds in its string
/// inputs (first capture group of every match, trimmed: the files a patch adds, updates, moves or
/// deletes). `[]` when the input is not an object or the tool has no pattern.
pub fn files_of(tool: Option<&str>, tool_input: &Json, tool_files: &IndexMap<String, Pattern>) -> Vec<String> {
    turn::files_in(tool, &Raw::new(tool_input, None), tool_files)
}

/// What one model call reported, in GitPrompt's names (the spec's `usage.values`). Numbers that are
/// missing or not numeric (numeric strings count) are 0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCall {
    pub model: Option<String>,
    /// Without cached input when the spec says the tool counts it in (`input_includes_cached`),
    /// never below 0.
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_read_tokens: f64,
    pub cache_creation_tokens: f64,
    pub reasoning_tokens: f64,
    pub cost_usd: f64,
    pub duration_ms: f64,
    pub query_source: Option<String>,
    /// The call reported an error (a non-empty, non-false `error` value).
    pub error: bool,
    /// completed, failed, cancelled... as the tool reported it.
    pub status: Option<String>,
}

/// Model call payloads (the spec's `usage.events`) in GitPrompt's names.
pub fn model_calls(payloads: &[&Json], usage: &Usage) -> Vec<ModelCall> {
    let payloads: Vec<Raw<'_>> = payloads.iter().map(|p| Raw::new(p, None)).collect();
    turn::calls_of(&payloads, usage)
}

/// Who a model call worked for, from telemetry `api_request.query_source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkKind {
    /// The main thread answering you: no query source, `repl_main_thread`, `sdk`.
    Main,
    /// A subagent: `agent:...`.
    Subagent,
    /// A call the tool makes on its own: compaction, titles, summaries, suggestions...
    System,
}

impl WorkKind {
    /// `main`, `subagent`, `system`: the prompt index `work` column.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkKind::Main => "main",
            WorkKind::Subagent => "subagent",
            WorkKind::System => "system",
        }
    }
}

pub fn work_kind(query_source: Option<&str>) -> WorkKind {
    turn::work_kind(query_source)
}

/// Requests, tokens and cost of some model calls: tokens summed then truncated to integers, cost
/// rounded to 4 decimals.
pub fn usage_of(calls: &[ModelCall]) -> UsageTotals {
    turn::usage_of(calls)
}

/// What an agent tool was asked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubagentTask {
    /// The first of [`TASK_INPUTS`] with a value.
    pub task: Option<String>,
    /// The first of [`BRIEF_INPUTS`] with a value, clipped to [`TEXT_CHARS`].
    pub brief: Option<String>,
}

/// An error the tool reported during the turn ([`ERROR_EVENTS`]). Serialized compacted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnError {
    /// `error.name`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `error.message`, else `error` itself; clipped to [`TEXT_CHARS`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// `error_context`, e.g. `model_call`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Json>,
    /// `recoverable`; `None` when absent or false (compacted away).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable: Option<bool>,
}

/// What happened while the tool worked on one prompt. Counts keyed by name keep `None` for events
/// that didn't name one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnSummary {
    // position
    /// Position of the prompt in its session.
    pub turn: Option<i64>,
    /// Seconds since the previous prompt of the session (1 decimal).
    pub since_previous_prompt_s: Option<f64>,

    // ending: only once a turn-ending event arrived
    /// Seconds from the prompt to the last turn-ending event (1 decimal).
    pub response_s: Option<f64>,
    /// `Stop`, `StopFailure` or `Interrupt`: the last turn-ending event.
    pub ended_by: Option<String>,
    /// What the end event reported went wrong ([`ERROR_KEYS`]).
    pub error: Option<Json>,
    /// The final answer: the latest event of the turn named in the spec's `answer` whose path holds a
    /// non-empty value (default: `Stop.last_assistant_message`).
    pub response: Option<Json>,

    // tools (finished calls: PostToolUse*)
    /// Calls per tool name, in first-seen order.
    pub tool_calls: IndexMap<Option<String>, u64>,
    pub tool_failures: u64,
    /// Sum of `duration_ms` of finished calls, in seconds (1 decimal).
    pub tool_time_s: f64,
    /// Sorted, unique paths touched by write tools.
    pub files_written: Vec<String>,
    /// Sorted, unique paths touched by read tools.
    pub files_read: Vec<String>,
    /// `command` inputs of shell tools, in order.
    pub bash_commands: Vec<String>,
    /// `url`, else `query`, of web tools.
    pub web: Vec<Option<String>>,
    /// Sorted, unique names of MCP tools used.
    pub mcp_tools: Vec<String>,
    /// Calls per kind (spec `tool_kinds`; unmapped tools are `other`).
    pub tool_kinds: IndexMap<ToolKind, u64>,
    /// Each named tool's kind.
    pub tool_kind_of: IndexMap<String, ToolKind>,
    /// Finished calls whose payload has an `agent_id` (made by a subagent).
    pub subagent_tool_calls: u64,
    /// Shell commands matching [`GIT_COMMIT`].
    pub git_commits: u64,
    /// What failed calls reported ([`ERROR_KEYS`], clipped); calls that reported nothing are skipped.
    pub tool_errors: Vec<String>,
    /// Inputs of agent tools that name a task or a brief.
    pub subagent_tasks: Vec<SubagentTask>,

    // agentic activity
    /// `command_name` of the first `UserPromptExpansion` or telemetry `user_prompt` event that has
    /// one, as the tool reported it (no slash added here).
    pub slash_command: Option<String>,
    /// `SubagentStart` events per `agent_type`.
    pub subagents: IndexMap<Option<String>, u64>,
    /// `TaskCreated.task_subject`, in order.
    pub tasks: Vec<Option<String>>,
    /// `Notification` events per `notification_type`.
    pub notifications: IndexMap<Option<String>, u64>,
    /// `PermissionDenied.tool_name`, in order.
    pub permission_denied: Vec<Option<String>>,
    /// `PreCompact` events.
    pub compactions: u64,
    /// `PostModelSwitch.to_model`, in order.
    pub model_switches: Vec<Option<String>>,
    /// `PreCompact` events per `trigger` (auto, manual); events without one are not counted.
    pub compaction_triggers: IndexMap<String, u64>,
    pub errors: Vec<TurnError>,

    // model calls (the spec's `usage` events)
    /// Requests, tokens (input without cached) and cost of every call.
    pub usage: UsageTotals,
    pub reasoning_tokens: i64,
    /// Calls per reported status; calls without one are not counted.
    pub model_call_status: IndexMap<String, u64>,
    /// Calls per model.
    pub models: IndexMap<Option<String>, u64>,
    /// The model that answered: of the main thread's calls ([`WorkKind::Main`]) with a model, the
    /// model of most calls, a tie going to the one called last. Background and subagent calls never
    /// count, however many there are. `None` without such calls. Not serialized: `facts.model`
    /// shows it (falling back to the model the hooks reported).
    pub main_model: Option<String>,
    /// Requests, tokens and cost per model.
    pub model_usage: IndexMap<Option<String>, UsageTotals>,
    /// Requests, tokens and cost per `query_source`.
    pub work_usage: IndexMap<Option<String>, UsageTotals>,
    /// Sum of the calls' `duration_ms`, in seconds (1 decimal).
    pub api_time_s: f64,
    /// Telemetry `api_error` events plus calls that reported an error.
    pub api_errors: u64,
    /// Telemetry `tool_decision` events with `decision: reject`.
    pub tool_rejections: u64,
}

/// Compacted, with the usage totals at the top level (`api_requests`, `input_tokens`, ...), as
/// `PromptOut.summary`.
impl Serialize for TurnSummary {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serial::turn_summary(self).serialize(s)
    }
}

/// What happened while the tool worked on one prompt. Without a spec every tool is `other`, no
/// event counts as a model call and no event carries an answer. Timestamps are ISO-8601 with an
/// offset or `Z`; one that can't be parsed leaves its duration out.
pub fn turn(prompt: &PromptView, events: &[EventRow], spec: Option<&Spec>) -> TurnSummary {
    let events = raw::events(events, None);
    turn::summary(&turn::Turn::new(prompt, &events, turn::Rules::of(spec)))
}

// --- the whole session: every event that shares the prompt's session_id ----------------------

/// A subagent that finished (`SubagentStop`). Serialized compacted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubagentResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// `agent_type`, else `subagent_type`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// `stop_reason`, else `status`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// `last_assistant_message`, else `summary`; clipped to [`TEXT_CHARS`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

/// Session-wide facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionSummary {
    /// Events per name.
    pub events: IndexMap<Option<String>, u64>,
    pub first_event_at: Option<String>,
    pub last_event_at: Option<String>,
    /// `SessionStart.source` of each start that has one: startup, then resume...
    pub starts: Vec<String>,
    /// `SessionEnd.reason` of the last session end.
    pub ended: Option<String>,
    /// Sorted, unique `InstructionsLoaded.file_path`.
    pub instructions_loaded: Vec<String>,
    /// `CwdChanged.new_cwd`, in order.
    pub cwd_changes: Vec<Option<String>>,
    /// `ConfigChange.file_path`, else its `source`.
    pub config_changes: Vec<Option<String>>,
    /// `SessionStart.composer_mode` of each start that has one (Cursor).
    pub modes: Vec<String>,
    /// Some `SessionStart` had `is_background_agent`.
    pub background: bool,
    pub subagent_results: Vec<SubagentResult>,
    /// Telemetry metric totals by [`metric`] name, rounded to 4 decimals.
    pub metric_totals: IndexMap<String, f64>,
    /// The resource attributes ([`ENVIRONMENT`] prefixes) of the latest event that carries
    /// `session.id`.
    pub environment: JsonMap,
}

/// Compacted, as `PromptDetail.session`.
impl Serialize for SessionSummary {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serial::session_summary(self).serialize(s)
    }
}

/// Session-wide facts from every event of the session (oldest first).
pub fn session(events: &[EventRow]) -> SessionSummary {
    session::summary(&raw::events(events, None))
}

/// [`session()`], recording every payload key it reads (field coverage).
pub(crate) fn session_recorded(events: &[EventRow], reads: &Reads) -> SessionSummary {
    session::summary(&raw::events(events, Some(reads)))
}

/// `(name, value)` of a telemetry metric data point: the event name (it must start with
/// [`METRIC_PREFIX`]) plus `[type]` when the payload has a `type`, e.g.
/// `claude_code.lines_of_code.count[added]`. `None` for other events and for points without
/// `value`; a non-numeric value counts as 0.
pub fn metric(event: &EventRow) -> Option<(String, f64)> {
    session::metric_of(&raw::Event { row: event, raw: Raw::new(&event.raw, None) })
}

/// `(/command, its command_source)` of a telemetry slash command event ([`COMMAND_EVENT`] with a
/// `command_name`; exactly one leading slash).
pub fn command(event: &EventRow) -> Option<(String, Option<String>)> {
    session::command_of(&raw::Event { row: event, raw: Raw::new(&event.raw, None) })
}

// --- everything derived about one prompt -----------------------------------------------------

/// A stored prompt plus every value derived from it. The prompt API returns this, and the prompt
/// index ([`crate::indexer::entry`]) stores a projection of it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Derived {
    #[serde(flatten)]
    pub prompt: PromptView,
    #[serde(flatten)]
    pub classified: Classified,
    pub surface: Option<String>,
    /// From the prompt as received (`""` when it has no text).
    pub stats: PromptStats,
    pub summary: TurnSummary,
}

impl Derived {
    pub fn origin(&self) -> Origin {
        self.classified.origin
    }
}

/// Origin, typed text and surface (from the spec's rules), stats from the text, summary from the
/// events of its turn.
pub fn derive(prompt: &PromptView, events: &[EventRow], spec: Option<&Spec>) -> Derived {
    derive_in(prompt, events, spec, None)
}

/// [`derive()`], recording every payload key it reads (field coverage).
pub(crate) fn derive_recorded(prompt: &PromptView, events: &[EventRow], spec: Option<&Spec>, reads: &Reads) -> Derived {
    derive_in(prompt, events, spec, Some(reads))
}

fn derive_in(prompt: &PromptView, events: &[EventRow], spec: Option<&Spec>, reads: Option<&Reads>) -> Derived {
    let row = &prompt.row;
    let raw = Raw::new(&row.raw, reads);
    let events = raw::events(events, reads);
    Derived {
        prompt: prompt.clone(),
        classified: crate::origin::classify_in(row, &raw, &events, spec),
        surface: crate::surface::surface_in(row, &raw, &events, spec),
        stats: prompt_stats(row.prompt.as_deref().unwrap_or_default()),
        summary: turn::summary(&turn::Turn::new(prompt, &events, turn::Rules::of(spec))),
    }
}

/// Every derived value and what it means, for the Fields page: [`crate::origin::DOCS`],
/// [`crate::surface::DOCS`], `stats.*`, `summary.*`, `session.*`, [`crate::facts::DOCS`], plus
/// `commits / diffstat`, `context.git_*` and `context.tool_version`. Names sharing a meaning are
/// grouped with ` / ` (e.g. `stats.chars / words / lines / approx_tokens`).
pub fn docs() -> IndexMap<String, String> {
    let prefixed = |prefix: &'static str, docs: &'static [(&'static str, &'static str)]| {
        docs.iter().map(move |(name, meaning)| (format!("{prefix}.{name}"), (*meaning).to_owned()))
    };
    let plain =
        |docs: &'static [(&'static str, &'static str)]| docs.iter().map(|(name, meaning)| ((*name).to_owned(), (*meaning).to_owned()));
    plain(crate::origin::DOCS)
        .chain(plain(crate::surface::DOCS))
        .chain(prefixed("stats", stats::DOCS))
        .chain(prefixed("summary", turn::DOCS))
        .chain(prefixed("session", session::DOCS))
        .chain(plain(crate::facts::DOCS))
        .chain(plain(EXTRA_DOCS))
        .collect()
}

/// Values the prompt API adds beside the derivations.
const EXTRA_DOCS: &[(&str, &str)] = &[
    ("commits / diffstat", "commits made after the prompt, until the next prompt of the session"),
    ("context.git_*", "repository state when the prompt was sent"),
    ("context.tool_version", "tool version when the prompt was sent"),
];
