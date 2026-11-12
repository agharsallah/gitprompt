//! Statistics over the prompt index: counts, sums and response-time quantiles for every prompt the
//! filter keeps, optionally grouped, each group optionally split by a second grouping (ADR-0012,
//! ADR-0014). For `model`, `tool`, `tool_kind`, `work`, `query_source` a prompt counts in every group
//! it touched, and the usage (or tool-call) measures are split exactly between them.

mod aggregate;
mod groups;
mod sessions;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::{Error, Result};

use crate::Store;
use crate::filter::PromptFilter;

/// Session length buckets, in seconds (lower bounds).
pub const SESSION_LENGTHS: &[u32] = &[0, 60, 300, 900, 1800, 3600, 7200, 14400];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// Local calendar day, YYYY-MM-DD (ordered by key).
    Day,
    /// `YYYY-MM-DD HH` (ordered by key).
    DayHour,
    /// `00`..`23` (ordered by key).
    Hour,
    /// `1`..`7` (ordered by key).
    Weekday,
    /// e.g. `1-09` (ordered by key).
    WeekdayHour,
    Origin,
    Source,
    Surface,
    /// detail: the session's title, else the start of its first prompt; measure `span_s`.
    Session,
    /// Sessions by duration bucket (key = zero-padded lower bound, ordered by key).
    SessionLength,
    /// detail: owner/name of the remote.
    Repo,
    Branch,
    Command,
    /// Typed-character bucket (ordered by key).
    Size,
    /// The same typed text (text hash); detail: the start of its last occurrence.
    Text,
    Outcome,
    Model,
    /// detail: the tool's kind.
    Tool,
    ToolKind,
    Work,
    /// detail: its kind of work.
    QuerySource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "StatsOrder")]
pub enum Order {
    Key,
    Prompts,
    Sessions,
    CostUsd,
    LastAt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AggregateQuery {
    pub filter: PromptFilter,
    pub group_by: Option<GroupBy>,
    /// Must differ from `group_by`; at most one of the two may be multi-valued (model, tool,
    /// `tool_kind`, work, `query_source`), else `Error::Invalid`.
    pub split_by: Option<GroupBy>,
    /// Default: by key for time and size buckets, else by prompts (desc, then key).
    pub order: Option<Order>,
    /// Max groups (default 1000).
    pub limit: usize,
}

impl Default for AggregateQuery {
    fn default() -> Self {
        AggregateQuery { filter: PromptFilter::default(), group_by: None, split_by: None, order: None, limit: 1000 }
    }
}

/// The measures of one bucket (totals, a group or a part). Absent measures are 0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(rename = "StatsBucket")]
pub struct Bucket {
    pub key: Option<String>,
    pub detail: Option<String>,
    pub prompts: i64,
    pub sessions: i64,
    pub api_requests: f64,
    pub cost_usd: f64,
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_read_tokens: f64,
    pub cache_creation_tokens: f64,
    pub api_time_s: f64,
    pub api_errors: f64,
    pub tool_calls: f64,
    pub tool_failures: f64,
    pub tool_time_s: f64,
    pub subagent_tool_calls: f64,
    pub subagents: f64,
    pub files_written: f64,
    pub git_commits: f64,
    pub compactions: f64,
    pub denials: f64,
    pub rejections: f64,
    /// Prompts whose turn has an end.
    pub ended: i64,
    pub failed: f64,
    /// Prompts with a later prompt in their session.
    pub followed: f64,
    pub with_code: f64,
    pub with_url: f64,
    pub with_mention: f64,
    pub with_secret: f64,
    pub with_commit: f64,
    pub distinct_texts: i64,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
    /// Linear-interpolated, 1 decimal; `None` without ended turns.
    pub response_s_p50: Option<f64>,
    pub response_s_p90: Option<f64>,
    /// Session groups only; `null` otherwise (always on the wire, as the dashboard's types expect).
    pub span_s: Option<f64>,
    /// With `split_by`: the same measures per value of the second grouping; `null` otherwise.
    pub parts: Option<Vec<Bucket>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Aggregate {
    pub totals: Bucket,
    pub groups: Vec<Bucket>,
}

/// Linear-interpolated quantile of sorted values, rounded to 1 decimal; `None` for no values.
pub fn quantile(sorted: &[f64], q: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let position = last as f64 * q.clamp(0.0, 1.0);
    // In 0..=last: `position` is a non-negative float no larger than `last`.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "position is in 0..=last, a valid index")]
    let lo = (position.floor() as usize).min(last);
    let hi = (lo + 1).min(last);
    let (low, high) = (*sorted.get(lo)?, *sorted.get(hi)?);
    let value = low + (high - low) * (position - lo as f64);
    Some(gitprompt_core::numbers::round(value, 1))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CommandUse {
    pub command: String,
    pub source: Option<String>,
    pub uses: i64,
    pub sessions: i64,
}

/// Per-session telemetry over the sessions of the prompts a filter keeps.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Telemetry {
    pub sessions: i64,
    /// metric name -> total, rounded to 4 decimals.
    pub metrics: indexmap::IndexMap<String, f64>,
    /// Most used first.
    pub commands: Vec<CommandUse>,
}

/// The newest prompt of one (connector, surface).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Latest {
    pub id: i64,
    pub source: String,
    pub surface: Option<String>,
    pub received_at: String,
    pub session: Option<String>,
    pub repo: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub origin: String,
    pub title: Option<String>,
    pub prompts: i64,
}

impl Store {
    /// Totals of the prompts the filter keeps; with `group_by`, the same per group, and with
    /// `split_by`, each group's parts. An impossible split is `Error::Invalid`.
    pub fn aggregate(&self, query: &AggregateQuery) -> Result<Aggregate> {
        check_split(query)?;
        let running_since = self.running_since();
        Ok(self.db().read(|conn| aggregate::aggregate(conn, query, &running_since))?)
    }

    /// Metric totals and slash commands over the sessions of the prompts the filter keeps, within
    /// `since`/`until` when given.
    pub fn telemetry(&self, filter: &PromptFilter) -> Result<Telemetry> {
        let running_since = self.running_since();
        Ok(self.db().read(|conn| sessions::telemetry(conn, filter, &running_since))?)
    }

    /// For each connector and surface: its newest prompt and prompt count, newest first.
    pub fn latest(&self) -> Result<Vec<Latest>> {
        Ok(self.db().read(sessions::latest)?)
    }
}

/// `split_by` needs a different `group_by`, and at most one of them may be multi-valued.
fn check_split(query: &AggregateQuery) -> Result<()> {
    let Some(split_by) = query.split_by else {
        return Ok(());
    };
    let Some(group_by) = query.group_by else {
        return Err(Error::Invalid("split_by needs group_by".into()));
    };
    if split_by == group_by {
        return Err(Error::Invalid("split_by must differ from group_by".into()));
    }
    if groups::group(group_by, "").multi_valued && groups::group(split_by, "").multi_valued {
        return Err(Error::Invalid("group_by and split_by can't both be groupings with several values per prompt".into()));
    }
    Ok(())
}
