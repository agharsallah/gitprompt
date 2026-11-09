//! The prompt index: a cache of derived values so filters and statistics run in SQL (ADR-0012). It
//! can be dropped any time and is rebuilt from the raw rows (gitprompt-insights computes the values,
//! this module only stores them).

use serde::{Deserialize, Serialize};

use gitprompt_core::Result;
use gitprompt_core::spec::ToolKind;

/// Requests, tokens and cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub api_requests: i64,
    pub cost_usd: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
}

/// One `prompt_index` row (+ the typed text, which goes to the FTS table only).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IndexRow {
    pub id: i64,
    pub prompt_id: Option<String>,
    pub source: String,
    pub session: Option<String>,
    /// git root at ingest, else the working directory.
    pub repo: Option<String>,
    /// owner/name of the origin remote.
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub title: Option<String>,
    pub received_at: String,
    /// Local calendar day, YYYY-MM-DD.
    pub day: String,
    pub hour: u8,
    /// ISO weekday, 1 = Monday.
    pub weekday: u8,
    pub origin: String,
    pub chars: i64,
    pub text_hash: Option<String>,
    pub slash_command: Option<String>,
    pub code_blocks: i64,
    pub urls: i64,
    pub mentions: i64,
    pub secrets: i64,
    #[serde(flatten)]
    pub usage: UsageTotals,
    pub api_time_s: f64,
    pub api_errors: i64,
    pub response_s: Option<f64>,
    /// Stop, StopFailure, Interrupt, or none.
    pub ended_by: Option<String>,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub tool_time_s: f64,
    pub subagent_tool_calls: i64,
    pub subagents: i64,
    pub files_written: i64,
    pub git_commits: i64,
    pub compactions: i64,
    pub denials: i64,
    pub rejections: i64,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub tool_version: Option<String>,
    pub reasoning_tokens: i64,
    pub answered: bool,
    pub surface: Option<String>,
    /// Goes to `prompt_fts` (trigram), not `prompt_index`.
    pub typed_text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: Option<String>,
    #[serde(flatten)]
    pub usage: UsageTotals,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCalls {
    pub tool: Option<String>,
    pub kind: ToolKind,
    pub calls: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkUsage {
    /// main, subagent, system
    pub work: String,
    pub query_source: Option<String>,
    #[serde(flatten)]
    pub usage: UsageTotals,
}

/// Everything the index stores about one prompt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub row: IndexRow,
    pub models: Vec<ModelUsage>,
    pub tools: Vec<ToolCalls>,
    pub work: Vec<WorkUsage>,
}

/// A telemetry metric data point of a session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionMetric {
    pub event_id: i64,
    pub session: Option<String>,
    pub received_at: String,
    pub name: String,
    pub value: f64,
}

/// A slash command telemetry saw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCommand {
    pub event_id: i64,
    pub session: Option<String>,
    pub received_at: String,
    pub command: String,
    pub source: Option<String>,
}

/// The index tables of a store.
pub struct PromptIndex<'a> {
    pub(crate) _store: &'a crate::Store,
}

impl PromptIndex<'_> {
    /// Create the tables (idempotent). Needs FTS5 with the trigram tokenizer.
    pub fn create(&self) -> Result<()> {
        todo!()
    }
    /// Forget everything (the index is a cache); `create` brings back empty tables.
    pub fn drop_all(&self) -> Result<()> {
        todo!()
    }
    /// Mark the whole index stale: the next `ensure` rebuilds it.
    pub fn invalidate(&self) -> Result<()> {
        todo!()
    }
    /// The rules version the rows were computed with.
    pub fn version(&self) -> Result<Option<String>> {
        todo!()
    }
    pub fn set_version(&self, _value: &str) -> Result<()> {
        todo!()
    }
    /// Replace the rows of these prompts (index, model, tool, work, FTS). Returns how many.
    pub fn upsert(&self, _entries: &[Entry]) -> Result<usize> {
        todo!()
    }
    pub fn delete(&self, _ids: &[i64]) -> Result<()> {
        todo!()
    }
    /// The last event id read for session telemetry (0 at first).
    pub fn telemetry_after(&self) -> Result<i64> {
        todo!()
    }
    pub fn add_telemetry(&self, _last_event: i64, _metrics: &[SessionMetric], _commands: &[SessionCommand]) -> Result<()> {
        todo!()
    }
    /// Note that these turns changed (cheap: done on every ingest). Idempotent.
    pub fn mark(&self, _prompt_ids: &[String]) -> Result<()> {
        todo!()
    }
    /// The marked turns, unmarked.
    pub fn take_marked(&self) -> Result<Vec<String>> {
        todo!()
    }
    /// Prompts stored after the newest indexed one.
    pub fn unindexed(&self) -> Result<Vec<i64>> {
        todo!()
    }
    /// The stored index row of a prompt (tests, the prompt API's facts), with `typed_text` read
    /// back from `prompt_fts`.
    pub fn row(&self, _id: i64) -> Result<Option<IndexRow>> {
        todo!()
    }
    /// Everything stored for a prompt: its row plus its model, tool and work rows (in insert order).
    pub fn entry(&self, _id: i64) -> Result<Option<Entry>> {
        todo!()
    }
}
