//! The summaries as the Python API serialized them: one JSON object per summary, keys in the order
//! the derivations produced them, then compacted (`gitprompt_core::json::compact`: null, `""`, `[]`,
//! `{}`, `false` and `0` left out). A `None` map key is written `"null"`.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::json;

use gitprompt_core::json::compact;
use gitprompt_core::spec::ToolKind;
use gitprompt_core::{Json, JsonMap};
use gitprompt_store::index::UsageTotals;

use super::{PromptStats, SessionSummary, TurnSummary};

/// Builds an object in insertion order.
struct Object(JsonMap);

impl Object {
    fn new() -> Self {
        Object(JsonMap::new())
    }

    fn put(mut self, key: &str, value: impl Serialize) -> Self {
        // Every value given here is built from strings, numbers, maps with string keys and unit
        // enums, which serde_json always serializes; null stands in should that ever change.
        let value = serde_json::to_value(value).unwrap_or(Json::Null);
        self.0.insert(key.to_owned(), value);
        self
    }

    fn compacted(self) -> JsonMap {
        compact(&self.0)
    }
}

/// A map keyed by optional names: `None` as `"null"`, like Python's `json.dumps`.
fn by_name<V: Serialize>(map: &IndexMap<Option<String>, V>) -> JsonMap {
    map.iter()
        .map(|(key, value)| (key.clone().unwrap_or_else(|| "null".to_owned()), serde_json::to_value(value).unwrap_or(Json::Null)))
        .collect()
}

fn usage_by_name(map: &IndexMap<Option<String>, UsageTotals>) -> JsonMap {
    map.iter().map(|(key, totals)| (key.clone().unwrap_or_else(|| "null".to_owned()), usage(totals))).collect()
}

fn kinds<V: Serialize>(map: &IndexMap<ToolKind, V>) -> JsonMap {
    map.iter().map(|(kind, value)| (kind_name(*kind).to_owned(), serde_json::to_value(value).unwrap_or(Json::Null))).collect()
}

/// A tool kind's serde name.
pub(crate) fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Write => "write",
        ToolKind::Shell => "shell",
        ToolKind::Web => "web",
        ToolKind::Mcp => "mcp",
        ToolKind::Agent => "agent",
        ToolKind::Other => "other",
    }
}

/// Requests, tokens and cost in the order the Python API wrote them.
fn usage(u: &UsageTotals) -> Json {
    json!({
        "api_requests": u.api_requests,
        "input_tokens": u.input_tokens,
        "output_tokens": u.output_tokens,
        "cache_read_tokens": u.cache_read_tokens,
        "cache_creation_tokens": u.cache_creation_tokens,
        "cost_usd": u.cost_usd,
    })
}

pub(super) fn prompt_stats(s: &PromptStats) -> JsonMap {
    Object::new()
        .put("chars", s.chars)
        .put("words", s.words)
        .put("lines", s.lines)
        .put("approx_tokens", s.approx_tokens)
        .put("slash_command", &s.slash_command)
        .put("at_mentions", &s.at_mentions)
        .put("urls", &s.urls)
        .put("code_blocks", s.code_blocks)
        .put("questions", s.questions)
        .put("secrets", &s.secrets)
        .compacted()
}

pub(super) fn turn_summary(t: &TurnSummary) -> JsonMap {
    let mut out = Object::new()
        .put("turn", t.turn)
        .put("since_previous_prompt_s", t.since_previous_prompt_s)
        .put("response_s", t.response_s)
        .put("ended_by", &t.ended_by)
        .put("error", &t.error)
        .put("response", &t.response)
        .put("tool_calls", by_name(&t.tool_calls))
        .put("tool_failures", t.tool_failures)
        .put("tool_time_s", t.tool_time_s)
        .put("files_written", &t.files_written)
        .put("files_read", &t.files_read)
        .put("bash_commands", &t.bash_commands)
        .put("web", &t.web)
        .put("mcp_tools", &t.mcp_tools)
        .put("tool_kinds", kinds(&t.tool_kinds))
        .put("tool_kind_of", t.tool_kind_of.iter().map(|(name, kind)| (name.clone(), kind_name(*kind))).collect::<IndexMap<_, _>>())
        .put("subagent_tool_calls", t.subagent_tool_calls)
        .put("git_commits", t.git_commits)
        .put("tool_errors", &t.tool_errors)
        .put("subagent_tasks", &t.subagent_tasks)
        .put("slash_command", &t.slash_command)
        .put("subagents", by_name(&t.subagents))
        .put("tasks", &t.tasks)
        .put("notifications", by_name(&t.notifications))
        .put("permission_denied", &t.permission_denied)
        .put("compactions", t.compactions)
        .put("model_switches", &t.model_switches)
        .put("compaction_triggers", &t.compaction_triggers)
        .put("errors", &t.errors);
    if let Json::Object(totals) = usage(&t.usage) {
        out.0.extend(totals);
    }
    out.put("reasoning_tokens", t.reasoning_tokens)
        .put("model_call_status", &t.model_call_status)
        .put("models", by_name(&t.models))
        .put("model_usage", usage_by_name(&t.model_usage))
        .put("work_usage", usage_by_name(&t.work_usage))
        .put("api_time_s", t.api_time_s)
        .put("api_errors", t.api_errors)
        .put("tool_rejections", t.tool_rejections)
        .compacted()
}

pub(super) fn session_summary(s: &SessionSummary) -> JsonMap {
    Object::new()
        .put("events", by_name(&s.events))
        .put("first_event_at", &s.first_event_at)
        .put("last_event_at", &s.last_event_at)
        .put("modes", &s.modes)
        .put("background", s.background)
        .put("starts", &s.starts)
        .put("ended", &s.ended)
        .put("instructions_loaded", &s.instructions_loaded)
        .put("cwd_changes", &s.cwd_changes)
        .put("config_changes", &s.config_changes)
        .put("subagent_results", &s.subagent_results)
        .put("metric_totals", &s.metric_totals)
        .put("environment", &s.environment)
        .compacted()
}
