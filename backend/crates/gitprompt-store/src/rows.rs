//! Stored rows as the store returns them.

use serde::{Deserialize, Serialize};

use gitprompt_core::{Json, JsonMap};

/// A stored prompt (table `prompt`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptRow {
    pub id: i64,
    pub source: String,
    /// UTC, ISO-8601 with microseconds and `+00:00`.
    pub received_at: String,
    /// The mapped prompt text; `None` = the mapping found none (drift signal).
    pub prompt: Option<String>,
    /// Values extracted from `raw` by the source spec.
    pub fields: JsonMap,
    /// Observed by GitPrompt at ingest: git state, tool version.
    pub context: JsonMap,
    /// The payload exactly as received (after `discard` and optional secret hiding).
    pub raw: Json,
}

/// A prompt with its place in its session (`prompt_view`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptView {
    #[serde(flatten)]
    pub row: PromptRow,
    pub annotation: Annotation,
    /// 1-based position among the prompts of the same source and session.
    pub turn: i64,
    /// `received_at` of the previous prompt of the session.
    pub prev_at: Option<String>,
    /// `git_head` at the next prompt of the session.
    pub next_head: Option<String>,
}

/// A stored event that is not a prompt (table `event`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    pub id: i64,
    pub source: String,
    pub received_at: String,
    pub event: Option<String>,
    pub session_id: Option<String>,
    pub prompt_id: Option<String>,
    pub fields: JsonMap,
    /// Long strings clipped (`core::json::clip`).
    pub raw: Json,
}

/// The user's own notes on a prompt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Annotation {
    pub starred: bool,
    /// -1 (bad), 0 (unrated), 1 (good): the dashboard's wire values. Out-of-range values are
    /// rejected by the API (422), never stored.
    pub rating: i8,
    pub tags: Vec<String>,
    pub note: String,
}
