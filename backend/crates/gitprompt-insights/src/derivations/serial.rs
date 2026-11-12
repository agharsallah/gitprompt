//! The summaries as the API writes them: one typed wire struct per summary, fields in the order the
//! derivations produce them, compacted (null, `""`, `[]`, `{}`, `false` and `0` left out, the
//! [`Blank`] rule). A `None` map key is written `"null"`, like Python's `json.dumps`.
//!
//! These structs are the wire contract: the dashboard's TypeScript types are generated from them
//! (`just types`), every compacted field optional there.

use indexmap::IndexMap;
use serde::Serialize;
use ts_rs::TS;

use gitprompt_core::Blank;
use gitprompt_core::Json;
use gitprompt_core::spec::ToolKind;
use gitprompt_store::index::UsageTotals;

use super::{PromptStats, SessionSummary, SubagentResult, SubagentTask, TurnError, TurnSummary};

/// Requests, tokens and cost of model calls, every count present.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "Usage")]
pub struct UsageOut {
    pub api_requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cost_usd: f64,
}

impl From<&UsageTotals> for UsageOut {
    fn from(u: &UsageTotals) -> Self {
        UsageOut {
            api_requests: u.api_requests,
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_creation_tokens: u.cache_creation_tokens,
            cost_usd: u.cost_usd,
        }
    }
}

/// A map keyed by optional names, `None` as `"null"`.
fn by_name<V: Clone>(map: &IndexMap<Option<String>, V>) -> IndexMap<String, V> {
    map.iter().map(|(key, value)| (key.clone().unwrap_or_else(|| "null".to_owned()), value.clone())).collect()
}

fn usage_by_name(map: &IndexMap<Option<String>, UsageTotals>) -> IndexMap<String, UsageOut> {
    map.iter().map(|(key, totals)| (key.clone().unwrap_or_else(|| "null".to_owned()), totals.into())).collect()
}

/// What the prompt text itself says (see [`PromptStats`]), compacted.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "PromptStats")]
pub struct PromptStatsOut {
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub chars: usize,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub words: usize,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub lines: usize,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub approx_tokens: usize,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub slash_command: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub at_mentions: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub code_blocks: usize,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub questions: usize,
    /// Kinds of credentials found, never the values.
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub secrets: Vec<String>,
}

impl From<&PromptStats> for PromptStatsOut {
    fn from(s: &PromptStats) -> Self {
        PromptStatsOut {
            chars: s.chars,
            words: s.words,
            lines: s.lines,
            approx_tokens: s.approx_tokens,
            slash_command: s.slash_command.clone(),
            at_mentions: s.at_mentions.clone(),
            urls: s.urls.clone(),
            code_blocks: s.code_blocks,
            questions: s.questions,
            secrets: s.secrets.clone(),
        }
    }
}

/// What happened while the tool worked on one prompt (see [`TurnSummary`]), compacted, with the
/// usage totals at the top level.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "TurnSummary")]
pub struct TurnSummaryOut {
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub turn: Option<i64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub since_previous_prompt_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub response_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub ended_by: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub error: Option<Json>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub response: Option<Json>,
    /// Calls per tool name.
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_calls: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_failures: u64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_time_s: f64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub files_written: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub files_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub bash_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub web: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub mcp_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_kinds: IndexMap<ToolKind, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_kind_of: IndexMap<String, ToolKind>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub subagent_tool_calls: u64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub git_commits: u64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub subagent_tasks: Vec<SubagentTask>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub slash_command: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub subagents: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tasks: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub notifications: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub permission_denied: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub compactions: u64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub model_switches: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub compaction_triggers: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub errors: Vec<TurnError>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub api_requests: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub input_tokens: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub output_tokens: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub cache_read_tokens: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub cache_creation_tokens: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub cost_usd: f64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub reasoning_tokens: i64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub model_call_status: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub models: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub model_usage: IndexMap<String, UsageOut>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub work_usage: IndexMap<String, UsageOut>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub api_time_s: f64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub api_errors: u64,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub tool_rejections: u64,
}

impl From<&TurnSummary> for TurnSummaryOut {
    fn from(t: &TurnSummary) -> Self {
        let u = &t.usage;
        TurnSummaryOut {
            turn: t.turn,
            since_previous_prompt_s: t.since_previous_prompt_s,
            response_s: t.response_s,
            ended_by: t.ended_by.clone(),
            error: t.error.clone(),
            response: t.response.clone(),
            tool_calls: by_name(&t.tool_calls),
            tool_failures: t.tool_failures,
            tool_time_s: t.tool_time_s,
            files_written: t.files_written.clone(),
            files_read: t.files_read.clone(),
            bash_commands: t.bash_commands.clone(),
            web: t.web.clone(),
            mcp_tools: t.mcp_tools.clone(),
            tool_kinds: t.tool_kinds.clone(),
            tool_kind_of: t.tool_kind_of.clone(),
            subagent_tool_calls: t.subagent_tool_calls,
            git_commits: t.git_commits,
            tool_errors: t.tool_errors.clone(),
            subagent_tasks: t.subagent_tasks.clone(),
            slash_command: t.slash_command.clone(),
            subagents: by_name(&t.subagents),
            tasks: t.tasks.clone(),
            notifications: by_name(&t.notifications),
            permission_denied: t.permission_denied.clone(),
            compactions: t.compactions,
            model_switches: t.model_switches.clone(),
            compaction_triggers: t.compaction_triggers.clone(),
            errors: t.errors.clone(),
            api_requests: u.api_requests,
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_creation_tokens: u.cache_creation_tokens,
            cost_usd: u.cost_usd,
            reasoning_tokens: t.reasoning_tokens,
            model_call_status: t.model_call_status.clone(),
            models: by_name(&t.models),
            model_usage: usage_by_name(&t.model_usage),
            work_usage: usage_by_name(&t.work_usage),
            api_time_s: t.api_time_s,
            api_errors: t.api_errors,
            tool_rejections: t.tool_rejections,
        }
    }
}

/// Session-wide facts (see [`SessionSummary`]), compacted.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "SessionFacts")]
pub struct SessionSummaryOut {
    /// Events per name.
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub events: IndexMap<String, u64>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub first_event_at: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub last_event_at: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub background: bool,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub starts: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    #[ts(optional)]
    pub ended: Option<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub instructions_loaded: Vec<String>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub cwd_changes: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub config_changes: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub subagent_results: Vec<SubagentResult>,
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub metric_totals: IndexMap<String, f64>,
    /// Resource attributes of the latest event that carries `session.id`.
    #[serde(default, skip_serializing_if = "Blank::is_blank")]
    pub environment: gitprompt_core::JsonMap,
}

impl From<&SessionSummary> for SessionSummaryOut {
    fn from(s: &SessionSummary) -> Self {
        SessionSummaryOut {
            events: by_name(&s.events),
            first_event_at: s.first_event_at.clone(),
            last_event_at: s.last_event_at.clone(),
            modes: s.modes.clone(),
            background: s.background,
            starts: s.starts.clone(),
            ended: s.ended.clone(),
            instructions_loaded: s.instructions_loaded.clone(),
            cwd_changes: s.cwd_changes.clone(),
            config_changes: s.config_changes.clone(),
            subagent_results: s.subagent_results.clone(),
            metric_totals: s.metric_totals.clone(),
            environment: s.environment.clone(),
        }
    }
}
