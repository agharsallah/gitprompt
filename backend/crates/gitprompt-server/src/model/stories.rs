//! Stories (session, project, prompt lineage) and full-text search.

use serde::Serialize;
use ts_rs::TS;

use gitprompt_core::spec::Origin;
use gitprompt_insights::facts::Facts;
use gitprompt_insights::snippets::Segment;
use gitprompt_insights::stories::ProjectRef as StoryProject;
use gitprompt_store::filter::Outcome;

/// `GET /api/stories/prompt/{id}`: the prompt's retries and reuses.
pub use gitprompt_insights::stories::PromptLineage as Lineage;
/// `GET /api/commits/{hash}`: the commit a hash names (its trace, project and forge URL), or every
/// candidate of an ambiguous prefix.
pub use gitprompt_insights::stories::{CommitLookup, FoundCommit};
/// `GET /api/stories/project/{id}/commits`: its query (`q`, `group`) and its answer, every commit
/// with the prompt behind it.
pub use gitprompt_insights::stories::{CommitQuery, ProjectCommits};
/// `GET /api/stories/session/{id}`, `GET /api/stories/project/{id}`.
pub use gitprompt_insights::stories::{ProjectStory, SessionStory};

/// One match of `GET /api/search`.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct SearchHit {
    pub id: i64,
    pub received_at: String,
    pub session: Option<String>,
    pub title: Option<String>,
    pub project: Option<StoryProject>,
    pub branch: Option<String>,
    /// As indexed: an [`Origin`] name.
    #[ts(as = "Origin")]
    pub origin: String,
    pub outcome: Outcome,
    pub cost_usd: f64,
    pub facts: Option<Facts>,
    /// The typed text around the matches, matched words marked.
    pub snippet: Vec<Segment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct SearchResults {
    /// Every match (all pages).
    pub total: u64,
    /// Newest first.
    pub hits: Vec<SearchHit>,
}
