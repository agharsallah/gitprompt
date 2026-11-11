//! What an export is built from, as plain values: collected by the caller (the API, the desktop
//! shell, or [`crate::collect`]), then redacted and built. These types mirror the JSON the Python
//! daemon's story routes, `storage.stats.aggregate` and `storage.timeline.prompts` return, so the
//! redaction pass can work on their JSON form by key name (ADR-0018 §2) and give the same values
//! back: every field a redaction rule can drop is optional (or defaulted) here.
//!
//! These are this crate's own types on purpose: the insights crate's story types can convert into
//! them without this crate depending on its API.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::Json;
use gitprompt_store::PromptFilter;
use gitprompt_store::stats::Bucket;
use gitprompt_store::timeline::IndexedPrompt;

use crate::Kind;

/// Everything one export shows, before redaction.
#[allow(
    clippy::large_enum_variant,
    reason = "one value per export, built once and never stored in bulk: boxing would only make it clumsier"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExportInput {
    Session(SessionExport),
    Project(ProjectExport),
    Overview(OverviewExport),
    Prompts(PromptsExport),
}

impl ExportInput {
    pub fn kind(&self) -> Kind {
        match self {
            ExportInput::Session(_) => Kind::Session,
            ExportInput::Project(_) => Kind::Project,
            ExportInput::Overview(_) => Kind::Overview,
            ExportInput::Prompts(_) => Kind::Prompts,
        }
    }
}

// --- facts (insights/facts.py) -------------------------------------------------------------------

/// The tool that sent a prompt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connector {
    pub id: Option<String>,
    pub name: Option<String>,
    /// Short name for badges. `label` is a text key, but the redaction pass keeps it here
    /// ([`crate::redact::TOOL_KEYS`]).
    pub label: Option<String>,
}

/// Where a prompt was typed (CLI, VS Code, a desktop app...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Surface {
    pub id: Option<String>,
    pub name: Option<String>,
    pub icon: Option<String>,
}

/// Tokens a tool reported for a turn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_creation: i64,
    pub reasoning: i64,
    pub total: i64,
}

/// The facts of a prompt every page shows (ADR-0021). `None` = the tool reported none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Facts {
    pub connector: Option<Connector>,
    pub surface: Option<Surface>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub tool_version: Option<String>,
    pub api_requests: Option<i64>,
    pub tokens: Option<Tokens>,
    /// `None`, not 0, when the tool reported no cost.
    pub cost_usd: Option<f64>,
}

// --- stories -------------------------------------------------------------------------------------

/// A commit made during a turn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    /// Dropped while `drop_identity`.
    pub author: Option<String>,
    pub time: Option<String>,
    pub files: i64,
    pub insertions: i64,
    pub deletions: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diffstat {
    pub commits: i64,
    pub files: i64,
    pub insertions: i64,
    pub deletions: i64,
}

/// The project a session ran in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectRef {
    pub id: Option<String>,
    /// A project folder (a root key).
    pub repo: Option<String>,
    pub name: Option<String>,
    pub git: bool,
}

/// A prompt you typed in a session story.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chapter {
    pub id: i64,
    /// 1-based position in the session.
    pub turn: i64,
    pub received_at: String,
    pub origin: String,
    /// What you typed, clipped; `None` when the options left it out.
    pub text: Option<String>,
    pub clipped: bool,
    pub slash_command: Option<String>,
    pub branch: Option<String>,
    /// `done`, `failed`, `interrupted`, `superseded`, `running`, `open` (or a key from a newer version).
    pub outcome: String,
    pub response_s: Option<f64>,
    pub cost_usd: f64,
    pub tokens: i64,
    pub tool_calls: i64,
    pub tool_failures: i64,
    /// Tool calls per tool kind.
    pub tools: IndexMap<String, i64>,
    pub files_written: i64,
    /// Files the turn's write tools touched.
    pub files: Vec<String>,
    pub subagents: i64,
    pub git_commits: i64,
    pub commits: Vec<Commit>,
    pub diffstat: Option<Diffstat>,
    /// The id of the prompt this one retries.
    pub retry_of: Option<i64>,
    pub facts: Option<Facts>,
}

/// System activity between chapters: a task notification, agent message, subagent prompt,
/// compaction...
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marker {
    /// An origin key, `subagent_prompt` or `compaction`.
    pub kind: String,
    pub id: i64,
    pub received_at: String,
    /// e.g. the subagent's description (a text key).
    pub label: Option<String>,
    /// e.g. the subagent type.
    pub detail: Option<String>,
    /// The system text, clipped (a text key).
    pub excerpt: Option<String>,
    pub clipped: bool,
}

/// One item of a session story: a chapter, or consecutive markers folded into one group.
#[allow(clippy::large_enum_variant, reason = "a story holds at most a few hundred items: boxing chapters would only make them clumsier")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StoryItem {
    Chapter(Chapter),
    Markers { items: Vec<Marker> },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionTotals {
    pub chapters: i64,
    pub markers: i64,
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub files_written: i64,
    /// Outcome key -> chapters, in first-seen order.
    pub outcomes: IndexMap<String, i64>,
    pub retries: i64,
    pub diffstat: Option<Diffstat>,
}

/// One session as a narrative (ADR-0016).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionStory {
    pub session: String,
    /// The session's title (a text key).
    pub title: Option<String>,
    pub source: Option<String>,
    pub connector: Option<Connector>,
    /// Model -> prompts, in first-seen order.
    pub models: IndexMap<String, i64>,
    pub project: Option<ProjectRef>,
    pub branches: Vec<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub totals: SessionTotals,
    pub items: Vec<StoryItem>,
}

/// A session export: its story and the session's telemetry environment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionExport {
    /// When the export was collected (ISO-8601); shown in local time.
    pub generated_at: String,
    pub story: SessionStory,
    /// Telemetry resource attributes (`app.version`, `user.email`...).
    pub environment: IndexMap<String, String>,
}

/// What a phase (or a whole project) amounted to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Measures {
    pub prompts: i64,
    pub sessions: i64,
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub files_written: i64,
    pub outcomes: IndexMap<String, i64>,
    pub diffstat: Option<Diffstat>,
    pub retries: i64,
    pub median_chars: i64,
    pub with_mention: i64,
    pub with_code: i64,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhaseReason {
    #[default]
    Start,
    /// After a pause (`gap_s`).
    Gap,
    /// The branch changed (`branches[0]` is the new one).
    Branch,
}

/// A prompt you typed in a project story.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPrompt {
    pub id: i64,
    pub received_at: String,
    pub session: Option<String>,
    pub origin: String,
    pub branch: Option<String>,
    /// The start of what you typed; `None` when the options left it out.
    pub text: Option<String>,
    pub clipped: bool,
    pub chars: i64,
    pub outcome: String,
    pub response_s: Option<f64>,
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub tools: IndexMap<String, i64>,
    pub files_written: i64,
    pub git_commits: i64,
    pub commits: Vec<Commit>,
    pub retry_of: Option<i64>,
    pub facts: Option<Facts>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Phase {
    /// 1-based.
    pub index: i64,
    pub reason: PhaseReason,
    pub gap_s: Option<f64>,
    pub branches: Vec<String>,
    #[serde(flatten)]
    pub measures: Measures,
    pub items: Vec<ProjectPrompt>,
}

/// How a project got built, prompt by prompt (ADR-0016).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectStory {
    pub id: Option<String>,
    /// The project folder (a root key).
    pub repo: Option<String>,
    pub name: Option<String>,
    pub git: bool,
    /// Prompts of other origins in the project: counted, never shown.
    pub system: i64,
    pub totals: Measures,
    pub phases: Vec<Phase>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectExport {
    pub generated_at: String,
    pub story: ProjectStory,
}

// --- statistics ----------------------------------------------------------------------------------

/// The groupings an Overview export shows, each as `Store::aggregate` returns its groups.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewGroups {
    pub day: Vec<Bucket>,
    pub origin: Vec<Bucket>,
    pub model: Vec<Bucket>,
    pub tool_kind: Vec<Bucket>,
    pub outcome: Vec<Bucket>,
    /// Keys are project folders: they are also made relative ([`crate::prepare`] adds them as roots).
    pub repo: Vec<Bucket>,
    pub command: Vec<Bucket>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewExport {
    pub generated_at: String,
    /// The filters the Overview was made with (described in the subtitle).
    pub filters: PromptFilter,
    pub totals: Bucket,
    pub groups: OverviewGroups,
}

/// A prompt of a selection: a timeline row with its facts, and its stored payload when collected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptItem {
    #[serde(flatten)]
    pub prompt: IndexedPrompt,
    #[serde(default)]
    pub facts: Option<Facts>,
    /// Only collected when `drop_raw` is off.
    #[serde(default)]
    pub raw: Option<Json>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptsExport {
    pub generated_at: String,
    pub filters: PromptFilter,
    /// Prompts the filters keep (the selection holds at most [`crate::MAX_PROMPTS`], newest first).
    pub total: i64,
    pub prompts: Vec<PromptItem>,
}
