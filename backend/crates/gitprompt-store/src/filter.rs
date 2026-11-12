//! Prompt filters, shared by the prompt list, statistics, stories and search. Typed: an unknown
//! filter is a compile error rather than a runtime `ValueError`. Empty values are ignored.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::spec::{Origin, ToolKind};

/// How a turn ended, in the dashboard's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "TurnOutcome")]
pub enum Outcome {
    /// Ended by `Stop` (or any end other than below).
    Done,
    /// Ended by `StopFailure`.
    Failed,
    /// Ended by `Interrupt` (you stopped it).
    Interrupted,
    /// No end, and a later prompt came in the same session.
    Superseded,
    /// No end, sent in the last hour.
    Running,
    /// No end, older than an hour, nothing after it.
    Open,
}

/// What a prompt or its turn contains; every listed condition must hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Has {
    Code,
    Url,
    Mention,
    Secret,
    Commit,
    Subagent,
    ToolFailure,
    ApiError,
    /// Denied by the permission system or rejected by you.
    Denial,
    Compaction,
    /// No later prompt in the session.
    NoFollowUp,
}

/// Size buckets of typed characters: a key is the lower bound, zero-padded to 5 digits.
pub const SIZES: &[u32] = &[0, 20, 80, 200, 500, 1500];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptFilter {
    /// Every word must appear in the typed text, anywhere, ignoring case (short words too).
    pub q: Option<String>,
    pub origin: Vec<Origin>,
    /// Surface ids (where it was typed).
    pub surface: Vec<String>,
    pub model: Vec<String>,
    pub source: Option<String>,
    /// One or several session ids.
    pub session: Vec<String>,
    pub repo: Option<String>,
    /// ISO-8601 UTC, inclusive.
    pub since: Option<String>,
    /// ISO-8601 UTC, exclusive.
    pub until: Option<String>,
    /// Local ISO weekday, 1 = Monday.
    pub weekday: Option<u8>,
    /// Local hour 0-23 (0 is a value, not "empty").
    pub hour: Option<u8>,
    /// Slash command with its leading slash.
    pub command: Option<String>,
    pub tool: Option<String>,
    pub tool_kind: Option<ToolKind>,
    pub query_source: Option<String>,
    pub outcome: Vec<Outcome>,
    /// A size bucket key, e.g. `"00080"`.
    pub size: Option<String>,
    pub has: Vec<Has>,
    /// Paging cursor: prompts with a smaller id.
    pub before: Option<i64>,
    pub tag: Option<String>,
    pub starred: Option<bool>,
}

impl Outcome {
    /// The dashboard's name of the outcome (its serde name).
    pub const fn as_str(self) -> &'static str {
        match self {
            Outcome::Done => "done",
            Outcome::Failed => "failed",
            Outcome::Interrupted => "interrupted",
            Outcome::Superseded => "superseded",
            Outcome::Running => "running",
            Outcome::Open => "open",
        }
    }

    /// The outcome named `name` ([`Self::as_str`]).
    pub fn from_name(name: &str) -> Option<Outcome> {
        [Outcome::Done, Outcome::Failed, Outcome::Interrupted, Outcome::Superseded, Outcome::Running, Outcome::Open]
            .into_iter()
            .find(|o| o.as_str() == name)
    }
}
