//! One turn ([`TurnSummary`]): the prompt and the events that share its `prompt_id`. Each group of
//! values is one small function over the turn ([`Turn`]); what a tool does comes from the spec's
//! `tool_kinds`, `tool_files`, `usage` and `answer`, never from tool names in code.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use indexmap::IndexMap;

use gitprompt_core::spec::{Pattern, Spec, ToolKind, Usage, UsageValue, by_name};
use gitprompt_store::PromptView;
use gitprompt_store::index::UsageTotals;

use super::{
    BRIEF_INPUTS, ERROR_EVENTS, ERROR_KEYS, FILE_INPUTS, GIT_COMMIT, MAIN_QUERY_SOURCES, ModelCall, SUBAGENT_PREFIX, SubagentTask,
    TASK_INPUTS, TEXT_CHARS, TOOL_RESULTS, TURN_END, TurnError, TurnSummary, WorkKind,
};
use crate::raw::{Event, Raw};
use crate::text::{clip, number, round, text_of, truthy};
use crate::time::seconds_between;

/// What each value means, by name under `summary.` (the Fields page).
pub(super) const DOCS: &[(&str, &str)] = &[
    ("turn", "position of the prompt in its session"),
    ("since_previous_prompt_s", "think time since the previous prompt of the session"),
    ("response_s / ended_by / error", "time until Stop, StopFailure or Interrupt, and how it ended"),
    ("response", "the agent's final answer (the spec's `answer` events: Stop.last_assistant_message by default)"),
    ("tool_calls / tool_failures / tool_time_s", "tools used for the prompt (PostToolUse*)"),
    ("files_written / files_read", "file paths touched by write and read tools (a path input, or the spec's tool_files pattern)"),
    ("bash_commands / web", "shell commands run, URLs fetched and searches made"),
    ("mcp_tools", "MCP tools used"),
    ("tool_kinds / tool_kind_of", "calls per kind (spec tool_kinds: read, write, shell, web, mcp, agent, other), and each tool's kind"),
    ("subagent_tool_calls", "tool calls made by subagents (their hook payload has agent_id)"),
    ("git_commits", "git commits the agent ran (shell commands with `git commit`)"),
    ("tool_errors", "what the failed tool calls reported (PostToolUseFailure.error)"),
    ("subagent_tasks", "what each subagent was asked: task name and brief (agent tools' inputs)"),
    ("slash_command", "slash command the prompt expanded (UserPromptExpansion, or telemetry user_prompt.command_name)"),
    ("subagents / tasks / notifications / permission_denied / compactions / model_switches", "agentic activity"),
    ("compaction_triggers", "what started each compaction: auto (the context was full) or manual"),
    ("errors", "errors the tool reported during the turn (ErrorOccurred): name, message, where, recoverable"),
    ("api_requests / models / *_tokens / cost_usd / api_time_s / api_errors", "model calls (the spec's `usage` events)"),
    ("reasoning_tokens", "output tokens spent reasoning, when the tool reports them"),
    ("model_call_status", "model calls per status the tool reported: completed, failed, cancelled"),
    ("model_usage", "requests, tokens and cost per model"),
    ("work_usage", "requests, tokens and cost per query_source: the main thread, a subagent, or a system call (telemetry)"),
    ("tool_rejections", "tool calls you rejected (telemetry)"),
];

/// [`GIT_COMMIT`], compiled once.
static GIT_COMMIT_RE: LazyLock<Pattern> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "a constant pattern, compiled by every test run that counts commits")]
    Pattern::new(GIT_COMMIT).expect("GIT_COMMIT is a valid pattern")
});

/// The spec's data a turn summary reads (empty without a spec).
#[derive(Clone, Copy)]
pub(super) struct Rules<'a> {
    tool_kinds: Option<&'a IndexMap<String, ToolKind>>,
    tool_files: Option<&'a IndexMap<String, Pattern>>,
    usage: Option<&'a Usage>,
    answer: Option<&'a IndexMap<String, String>>,
}

impl<'a> Rules<'a> {
    pub(super) fn of(spec: Option<&'a Spec>) -> Self {
        Rules {
            tool_kinds: spec.map(|s| &s.tool_kinds),
            tool_files: spec.map(|s| &s.tool_files),
            usage: spec.map(|s| &s.usage),
            answer: spec.map(|s| &s.answer),
        }
    }

    fn kind(&self, tool: Option<&str>) -> ToolKind {
        self.tool_kinds.and_then(|kinds| by_name(kinds, tool)).copied().unwrap_or_default()
    }
}

/// A finished tool call.
struct ToolUse<'a> {
    name: Option<String>,
    /// `tool_input` when it has a value.
    input: Option<Raw<'a>>,
    kind: ToolKind,
}

impl<'a> ToolUse<'a> {
    /// The input value of `key`, when the input is an object that has a truthy one.
    fn input(&self, key: &str) -> Option<Raw<'a>> {
        self.input.as_ref()?.truthy(key)
    }
}

/// A prompt and the events of its turn, as the turn derivations read them.
pub(super) struct Turn<'a> {
    prompt: &'a PromptView,
    events: &'a [Event<'a>],
    rules: Rules<'a>,
    tool_uses: Vec<ToolUse<'a>>,
}

impl<'a> Turn<'a> {
    pub(super) fn new(prompt: &'a PromptView, events: &'a [Event<'a>], rules: Rules<'a>) -> Self {
        let tool_uses = events
            .iter()
            .filter(|e| e.is(TOOL_RESULTS))
            .map(|e| {
                let name = e.raw.get("tool_name").and_then(|n| text_of(n.json()));
                let kind = rules.kind(name.as_deref());
                ToolUse { input: e.raw.truthy("tool_input"), kind, name }
            })
            .collect();
        Turn { prompt, events, rules, tool_uses }
    }

    /// Payloads of the events with one of these names.
    fn raws<'s>(&'s self, names: &'s [&str]) -> impl Iterator<Item = &'s Raw<'a>> + 's {
        self.events.iter().filter(move |e| e.is(names)).map(|e| &e.raw)
    }

    /// The finished calls of tools of this kind.
    fn uses(&self, kind: ToolKind) -> impl Iterator<Item = &ToolUse<'a>> {
        self.tool_uses.iter().filter(move |t| t.kind == kind)
    }

    /// Sorted, unique files touched by the finished calls of tools of this kind.
    fn files(&self, kind: ToolKind) -> Vec<String> {
        let none = IndexMap::new();
        let patterns = self.rules.tool_files.unwrap_or(&none);
        let files: BTreeSet<String> =
            self.uses(kind).filter_map(|t| Some(files_in(t.name.as_deref(), t.input.as_ref()?, patterns))).flatten().collect();
        files.into_iter().collect()
    }
}

/// Every derivation of a turn, in the order the Python registry ran them.
pub(super) fn summary(t: &Turn<'_>) -> TurnSummary {
    let mut out = TurnSummary::default();
    position(t, &mut out);
    ending(t, &mut out);
    tools(t, &mut out);
    agent_activity(t, &mut out);
    usage(t, &mut out);
    out
}

fn position(t: &Turn<'_>, out: &mut TurnSummary) {
    out.turn = Some(t.prompt.turn);
    out.since_previous_prompt_s = t.prompt.prev_at.as_deref().and_then(|prev| seconds_between(prev, &t.prompt.row.received_at));
}

fn ending(t: &Turn<'_>, out: &mut TurnSummary) {
    let Some(end) = t.events.iter().rev().find(|e| e.is(TURN_END)) else { return };
    out.response_s = seconds_between(&t.prompt.row.received_at, &end.row.received_at);
    out.ended_by = end.name().map(String::from);
    out.error = end.raw.first(ERROR_KEYS).map(|e| e.json().clone());
    let answer = t.rules.answer;
    out.response = t
        .events
        .iter()
        .rev()
        .filter_map(|e| Some((e, answer?.get(e.name()?)?)))
        .find_map(|(e, path)| e.raw.pluck(path).filter(Raw::is_truthy))
        .map(|a| a.json().clone());
}

fn tools(t: &Turn<'_>, out: &mut TurnSummary) {
    for tool in &t.tool_uses {
        *out.tool_calls.entry(tool.name.clone()).or_default() += 1;
        *out.tool_kinds.entry(tool.kind).or_default() += 1;
        if let Some(name) = tool.name.as_ref().filter(|n| !n.is_empty()) {
            out.tool_kind_of.entry(name.clone()).or_insert(tool.kind);
        }
    }
    let results: Vec<&Raw<'_>> = t.raws(TOOL_RESULTS).collect();
    out.tool_failures = count(t.raws(&["PostToolUseFailure"]));
    out.tool_time_s = round(results.iter().map(|r| number(r.get("duration_ms").as_ref().map(Raw::json))).sum::<f64>() / 1000.0, 1);
    out.files_written = t.files(ToolKind::Write);
    out.files_read = t.files(ToolKind::Read);
    out.bash_commands = t.uses(ToolKind::Shell).filter_map(|u| u.input("command")).filter_map(|c| text_of(c.json())).collect();
    out.web =
        t.uses(ToolKind::Web).map(|u| u.input("url").or_else(|| u.input.as_ref()?.get("query")).and_then(|v| text_of(v.json()))).collect();
    let mcp: BTreeSet<String> = t.uses(ToolKind::Mcp).filter_map(|u| u.name.clone().filter(|n| !n.is_empty())).collect();
    out.mcp_tools = mcp.into_iter().collect();
    out.subagent_tool_calls = count(results.iter().filter(|r| r.truthy("agent_id").is_some()));
    out.git_commits = count(out.bash_commands.iter().filter(|c| GIT_COMMIT_RE.is_match(c)));
    out.tool_errors = t.raws(&["PostToolUseFailure"]).filter_map(|r| summary_text(r.first(ERROR_KEYS)?.json())).collect();
    out.subagent_tasks = t
        .uses(ToolKind::Agent)
        .filter_map(|u| {
            let input = u.input.as_ref()?;
            input.first(&[TASK_INPUTS, BRIEF_INPUTS].concat())?;
            Some(SubagentTask {
                task: input.first(TASK_INPUTS).and_then(|v| text_of(v.json())),
                brief: input.first(BRIEF_INPUTS).and_then(|v| summary_text(v.json())),
            })
        })
        .collect();
}

fn agent_activity(t: &Turn<'_>, out: &mut TurnSummary) {
    let value = |r: &Raw<'_>, key: &str| r.get(key).and_then(|v| text_of(v.json()));
    out.slash_command =
        t.raws(&["UserPromptExpansion", "user_prompt"]).find_map(|r| r.truthy("command_name")).and_then(|v| text_of(v.json()));
    for r in t.raws(&["SubagentStart"]) {
        *out.subagents.entry(value(r, "agent_type")).or_default() += 1;
    }
    out.tasks = t.raws(&["TaskCreated"]).map(|r| value(r, "task_subject")).collect();
    for r in t.raws(&["Notification"]) {
        *out.notifications.entry(value(r, "notification_type")).or_default() += 1;
    }
    out.permission_denied = t.raws(&["PermissionDenied"]).map(|r| value(r, "tool_name")).collect();
    out.compactions = count(t.raws(&["PreCompact"]));
    out.model_switches = t.raws(&["PostModelSwitch"]).map(|r| value(r, "to_model")).collect();
    for trigger in t.raws(&["PreCompact"]).filter_map(|r| r.truthy("trigger")).filter_map(|v| text_of(v.json())) {
        *out.compaction_triggers.entry(trigger).or_default() += 1;
    }
    out.errors = t.raws(ERROR_EVENTS).map(turn_error).collect();
}

/// An `ErrorOccurred` payload, with the values compaction would drop left out.
fn turn_error<'a>(r: &Raw<'a>) -> TurnError {
    let kept = |v: Option<Raw<'a>>| v.filter(Raw::is_truthy);
    TurnError {
        name: kept(r.pluck("error.name")).and_then(|v| text_of(v.json())),
        message: kept(r.pluck("error.message")).or_else(|| r.get("error")).and_then(|v| summary_text(v.json())),
        context: kept(r.get("error_context")).map(|v| v.json().clone()),
        recoverable: kept(r.get("recoverable")).map(|_| true),
    }
}

fn usage(t: &Turn<'_>, out: &mut TurnSummary) {
    // Without a spec no event is a model call.
    let none = Usage::default();
    let spec_usage = t.rules.usage.unwrap_or(&none);
    let names: Vec<&str> = spec_usage.events.iter().map(String::as_str).collect();
    let payloads: Vec<Raw<'_>> = t.raws(&names).cloned().collect();
    let calls = calls_of(&payloads, spec_usage);
    out.usage = usage_of(&calls);
    // Truncated like Python's int(): token counts are whole by nature; a fraction is the tool's.
    #[expect(clippy::cast_possible_truncation, reason = "truncation is the point: Python's int() of a token sum")]
    let reasoning = calls.iter().map(|c| c.reasoning_tokens).sum::<f64>() as i64;
    out.reasoning_tokens = reasoning;
    for status in calls.iter().filter_map(|c| c.status.clone()) {
        *out.model_call_status.entry(status).or_default() += 1;
    }
    for call in &calls {
        *out.models.entry(call.model.clone()).or_default() += 1;
    }
    out.main_model = main_thread_model(&calls);
    out.model_usage = grouped(&calls, |c| c.model.clone());
    out.work_usage = grouped(&calls, |c| c.query_source.clone());
    out.api_time_s = round(calls.iter().map(|c| c.duration_ms).sum::<f64>() / 1000.0, 1);
    out.api_errors = count(t.raws(&["api_error"])) + count(calls.iter().filter(|c| c.error));
    out.tool_rejections =
        count(t.raws(&["tool_decision"]).filter(|r| r.get("decision").is_some_and(|d| d.json().as_str() == Some("reject"))));
}

/// [`TurnSummary::main_model`]: decided by who each call worked for (`query_source`), never by
/// model names, so models and versions that don't exist yet need no change here.
fn main_thread_model(calls: &[ModelCall]) -> Option<String> {
    // model -> (calls, position of its latest call)
    let mut seen: IndexMap<&str, (usize, usize)> = IndexMap::new();
    let main = calls.iter().filter(|c| work_kind(c.query_source.as_deref()) == WorkKind::Main);
    for (at, model) in main.filter_map(|c| c.model.as_deref().filter(|m| !m.is_empty())).enumerate() {
        let entry = seen.entry(model).or_default();
        *entry = (entry.0 + 1, at);
    }
    seen.into_iter().max_by_key(|(_, rank)| *rank).map(|(model, _)| model.to_owned())
}

/// Usage per key, keys in first-seen order.
fn grouped(calls: &[ModelCall], key: impl Fn(&ModelCall) -> Option<String>) -> IndexMap<Option<String>, UsageTotals> {
    let mut groups: IndexMap<Option<String>, Vec<ModelCall>> = IndexMap::new();
    for call in calls {
        groups.entry(key(call)).or_default().push(call.clone());
    }
    groups.into_iter().map(|(key, group)| (key, usage_of(&group))).collect()
}

fn count<T>(items: impl Iterator<Item = T>) -> u64 {
    // usize -> u64 is lossless on every supported target.
    items.count() as u64
}

/// A message clipped for a summary; `None` for no value or `""`.
pub(super) fn summary_text(value: &gitprompt_core::Json) -> Option<String> {
    let text = text_of(value).filter(|t| !t.is_empty())?;
    Some(clip(&text, TEXT_CHARS).0)
}

/// [`super::files_of`] over a payload being read.
pub(super) fn files_in(tool: Option<&str>, input: &Raw<'_>, tool_files: &IndexMap<String, Pattern>) -> Vec<String> {
    if !input.json().is_object() {
        return Vec::new();
    }
    if let Some(path) = input.first(FILE_INPUTS).and_then(|p| text_of(p.json())) {
        return vec![path];
    }
    let Some(pattern) = by_name(tool_files, tool) else { return Vec::new() };
    input
        .entries()
        .into_iter()
        .filter_map(|(_, value)| value.json().as_str())
        .flat_map(|text| pattern.first_groups(text))
        .map(|file| crate::text::trim(&file).to_owned())
        .collect()
}

/// [`super::model_calls`] over payloads being read.
pub(super) fn calls_of(payloads: &[Raw<'_>], usage: &Usage) -> Vec<ModelCall> {
    payloads
        .iter()
        .map(|payload| {
            let mut call = ModelCall::default();
            for (value, path) in &usage.values {
                let found = payload.pluck(path);
                let json = found.as_ref().map(Raw::json);
                let numeric = number(json);
                match value {
                    UsageValue::Model => call.model = json.and_then(text_of),
                    UsageValue::InputTokens => call.input_tokens = numeric,
                    UsageValue::OutputTokens => call.output_tokens = numeric,
                    UsageValue::CacheReadTokens => call.cache_read_tokens = numeric,
                    UsageValue::CacheCreationTokens => call.cache_creation_tokens = numeric,
                    UsageValue::ReasoningTokens => call.reasoning_tokens = numeric,
                    UsageValue::CostUsd => call.cost_usd = numeric,
                    UsageValue::DurationMs => call.duration_ms = numeric,
                    UsageValue::QuerySource => call.query_source = json.and_then(text_of),
                    UsageValue::Error => call.error = json.is_some_and(truthy),
                    UsageValue::Status => call.status = json.filter(|s| truthy(s)).and_then(text_of),
                }
            }
            if usage.input_includes_cached {
                call.input_tokens = (call.input_tokens - call.cache_read_tokens).max(0.0);
            }
            call
        })
        .collect()
}

/// Requests, tokens (summed, then truncated like Python's `int()`) and cost (4 decimals).
pub(super) fn usage_of(calls: &[ModelCall]) -> UsageTotals {
    #[expect(clippy::cast_possible_truncation, reason = "truncation is the point: Python's int() of a token sum")]
    let tokens = |f: fn(&ModelCall) -> f64| calls.iter().map(f).sum::<f64>() as i64;
    UsageTotals {
        api_requests: i64::try_from(calls.len()).unwrap_or(i64::MAX),
        cost_usd: round(calls.iter().map(|c| c.cost_usd).sum(), 4),
        input_tokens: tokens(|c| c.input_tokens),
        output_tokens: tokens(|c| c.output_tokens),
        cache_read_tokens: tokens(|c| c.cache_read_tokens),
        cache_creation_tokens: tokens(|c| c.cache_creation_tokens),
    }
}

pub(super) fn work_kind(query_source: Option<&str>) -> WorkKind {
    match query_source {
        None => WorkKind::Main,
        Some(source) if MAIN_QUERY_SOURCES.contains(&source) => WorkKind::Main,
        Some(source) if source.starts_with("repl_main_thread:") => WorkKind::Main,
        Some(source) if source.starts_with(SUBAGENT_PREFIX) => WorkKind::Subagent,
        Some(_) => WorkKind::System,
    }
}
