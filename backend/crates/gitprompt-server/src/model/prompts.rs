//! The prompt list and detail, and the user's notes on a prompt.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::CommitsAfter;
use gitprompt_core::spec::Origin;
use gitprompt_core::{Json, JsonMap};
use gitprompt_insights::derivations::{PromptStats, SessionSummary, TurnSummary};
use gitprompt_insights::facts::Facts;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::rows::EventRow;

/// The user's notes on a prompt, as the dashboard sends and reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct Annotation {
    pub starred: bool,
    /// -1 (bad), 0 (unrated), 1 (good); anything else is a 422.
    pub rating: i8,
    pub tags: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Injected {
    /// The block's tag, e.g. `ide_selection`.
    pub kind: String,
    pub chars: u64,
}

/// A prompt in the list (`GET /api/prompts`).
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "Prompt")]
pub struct PromptOut {
    pub id: i64,
    pub source: String,
    pub received_at: String,
    /// As received, injected context included.
    pub prompt: Option<String>,
    pub origin: Origin,
    /// The prompt without injected context.
    pub typed_text: Option<String>,
    pub injected: Vec<Injected>,
    pub fields: JsonMap,
    pub context: JsonMap,
    pub annotation: Annotation,
    /// Derived from the prompt text.
    pub stats: PromptStats,
    /// Derived from the turn's events.
    pub summary: TurnSummary,
    pub facts: Facts,
}

/// One prompt with everything (`GET /api/prompts/{id}`).
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct PromptDetail {
    #[serde(flatten)]
    pub prompt: PromptOut,
    /// The payload as the tool sent it.
    #[ts(type = "Record<string, unknown>")]
    pub raw: Json,
    /// Derived over the session's events; empty for a prompt without a session.
    pub session: SessionSummary,
    /// The commits made in the prompt's checkout after it; empty outside a repository.
    pub commits: CommitsAfter,
    /// The turn's events, oldest first.
    pub events: Vec<EventRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Updated {
    pub updated: u64,
}

/// Query of `GET /api/prompts` (and `/api/search`): the prompt filters plus `limit` (1..=200,
/// default 50).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptQuery {
    pub filter: PromptFilter,
    pub limit: usize,
}

impl Default for PromptQuery {
    fn default() -> Self {
        PromptQuery { filter: PromptFilter::default(), limit: 50 }
    }
}

impl From<gitprompt_store::Annotation> for Annotation {
    fn from(a: gitprompt_store::Annotation) -> Self {
        Annotation { starred: a.starred, rating: a.rating, tags: a.tags, note: a.note }
    }
}

impl From<Annotation> for gitprompt_store::Annotation {
    fn from(a: Annotation) -> Self {
        gitprompt_store::Annotation { starred: a.starred, rating: a.rating, tags: a.tags, note: a.note }
    }
}
