//! Wire types: what each route answers and the Service returns, mirroring the FastAPI response
//! models (`api/routes/*.py`) and `web/src/lib/api/types.ts`. Field names and optionality are the
//! wire contract with the dashboard; change them only together with `types.ts`.
//!
//! Shapes computed by the insights crate (prompt stats and turn summary, session derivations, the
//! field catalog, stories, lineage) are carried as JSON values here and documented by their Python
//! models; they can become re-exports of the insights types without changing the wire.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::preferences::{Applies, ValueSource};
use gitprompt_core::spec::{Capability, Join, Origin, SpecOrigin, SurfaceRef};
use gitprompt_core::{Json, JsonMap};
use gitprompt_store::filter::{Outcome, PromptFilter};
use gitprompt_store::rows::EventRow;
use gitprompt_store::stats::{Aggregate, GroupBy, Order};

// --- prompts ---

/// The user's notes on a prompt, as the dashboard sends and reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Annotation {
    pub starred: bool,
    /// -1 (bad), 0 (unrated), 1 (good); anything else is a 422.
    pub rating: i8,
    pub tags: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Injected {
    /// The block's tag, e.g. `ide_selection`.
    pub kind: String,
    pub chars: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    /// The spec id (`source`).
    pub id: String,
    pub name: String,
    /// The name on a badge (spec `short_name`).
    pub label: String,
    /// Who answers, e.g. "Codex".
    pub assistant: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_creation: i64,
    pub reasoning: i64,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    Usage,
    Hook,
}

/// What every page shows about a turn (insights facts, ADR-0021).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub connector: Connector,
    /// `null`: the tool doesn't say where the prompt was typed.
    pub surface: Option<SurfaceRef>,
    pub model: Option<String>,
    pub models: Vec<String>,
    pub model_source: Option<ModelSource>,
    pub tokens: Option<Tokens>,
    pub cost_usd: Option<f64>,
    pub api_requests: i64,
    pub outcome: Option<Outcome>,
    pub permission_mode: Option<String>,
    pub tool_version: Option<String>,
    pub response_s: Option<f64>,
    pub answered: bool,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub files_written: i64,
    pub git_commits: i64,
    pub subagents: i64,
}

/// A prompt in the list (`GET /api/prompts`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Derived from the prompt text (`derivations.PROMPT_STATS`).
    pub stats: JsonMap,
    /// Derived from the turn's events (`derivations.TURN`).
    pub summary: JsonMap,
    pub facts: Facts,
}

/// One prompt with everything (`GET /api/prompts/{id}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PromptDetail {
    #[serde(flatten)]
    pub prompt: PromptOut,
    pub raw: Json,
    /// `derivations.SESSION` over the session's events.
    pub session: JsonMap,
    /// `core::git::CommitsAfter` in the prompt's checkout, `{}` outside a repository.
    pub commits: JsonMap,
    /// The turn's events, oldest first.
    pub events: Vec<EventRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

// --- sources ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFileOut {
    pub name: String,
    pub path: String,
    pub origin: SpecOrigin,
    pub overrides: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecProblemOut {
    pub file: SpecFileOut,
    pub error: String,
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedSurface {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// What of this app reaches the connector (spec `captures`).
    pub note: String,
}

/// Activity window of `SourceOut::hourly`: prompts + events per UTC hour, the current hour last.
pub const HOURS: usize = 24;

/// One connector with the raw facts the dashboard derives its state from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceOut {
    pub id: String,
    pub name: String,
    pub assistant: String,
    pub part_of: Option<String>,
    pub layer: String,
    pub summary: String,
    /// `false`: turned off in GitPrompt (preference `disabled_sources`).
    pub enabled: bool,
    pub detected: bool,
    pub version: Option<String>,
    pub connected: bool,
    pub outdated: bool,
    pub config_file: String,
    pub terms: String,
    pub after_connect: String,
    pub blocks: Vec<String>,
    pub warnings: Vec<String>,
    pub total: u64,
    pub unmapped: u64,
    pub events: u64,
    pub last_at: Option<String>,
    /// [`HOURS`] counts, oldest first, the current UTC hour (by the app clock) last.
    pub hourly: Vec<u64>,
    pub collects: Vec<String>,
    pub not_stored: IndexMap<String, String>,
    pub not_collected: IndexMap<String, String>,
    pub capabilities: Vec<Capability>,
    pub needs_version: IndexMap<Capability, String>,
    pub join: Join,
    pub captures: Vec<CapturedSurface>,
    pub cannot_capture: IndexMap<String, String>,
    pub spec: SpecFileOut,
    pub spec_problem: Option<SpecProblemOut>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub repo: String,
    pub name: String,
}

/// The newest prompt of one connector and surface (`GET /api/sources/recent`), newest first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentCapture {
    pub id: i64,
    pub connector: Connector,
    pub surface: Option<SurfaceRef>,
    pub received_at: String,
    pub session: Option<String>,
    pub title: Option<String>,
    pub project: Option<ProjectRef>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub origin: Origin,
    pub prompts: i64,
    /// What the surface rules read, e.g. `{"app.entrypoint": "cli"}`.
    pub signals: JsonMap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecText {
    pub file: SpecFileOut,
    pub text: String,
}

/// A connect/disconnect (or its dry run): the unified diff of the tool's config file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub file: String,
    /// Empty when there is nothing to change.
    pub diff: String,
    pub applied: bool,
    pub backup: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enabled {
    pub enabled: bool,
}

// --- stats ---

/// Query of `GET /api/stats`: the prompt filters plus grouping. `limit` 1..=10000 (default 1000).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatsQuery {
    pub filter: PromptFilter,
    pub group_by: Option<GroupBy>,
    pub split_by: Option<GroupBy>,
    pub order: Option<Order>,
    pub limit: usize,
}

impl Default for StatsQuery {
    fn default() -> Self {
        StatsQuery { filter: PromptFilter::default(), group_by: None, split_by: None, order: None, limit: 1000 }
    }
}

/// `GET /api/stats`. Group `detail` names a `source` group by its badge (spec `short_name`) and a
/// `surface` group by its label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub group_by: Option<GroupBy>,
    pub split_by: Option<GroupBy>,
    #[serde(flatten)]
    pub aggregate: Aggregate,
}

pub use gitprompt_store::stats::Telemetry;

// --- fields ---

/// `GET /api/fields` (insights catalog): `{fields: [{source, event, path, count, sample,
/// documented, unused}], not_captured: [{source, event, reason}], derived: {name: meaning}}`.
pub type Catalog = Json;

// --- settings ---

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingOut {
    pub value: Json,
    pub default: Json,
    pub source: ValueSource,
    pub env: String,
    pub locked: bool,
    pub applies: Applies,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsOut {
    /// `<home>/preferences.json`.
    pub file: String,
    /// Every preference key, in `KEYS` order.
    pub settings: IndexMap<String, SettingOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsSaved {
    #[serde(flatten)]
    pub current: SettingsOut,
    pub changed: Vec<String>,
    pub restart_required: bool,
}

// --- data ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backup {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEvents {
    pub source: String,
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataOut {
    pub home: String,
    pub db_path: String,
    pub db_bytes: u64,
    pub receiver: String,
    /// Stored raw events per source, largest first.
    pub events: Vec<SourceEvents>,
    /// Config-file copies taken before each connect/disconnect, newest first.
    pub backups: Vec<Backup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalOut {
    pub source: String,
    pub event: Option<String>,
    pub rows: u64,
    pub bytes: u64,
    /// "dropped by spec", "expired" or "selected".
    pub why: String,
    pub before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportOut {
    pub removals: Vec<RemovalOut>,
    pub rows: u64,
    pub bytes: u64,
    pub size_before: u64,
    pub size_after: Option<u64>,
    pub applied: bool,
}

/// Body of `POST /api/data/compact`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactIn {
    /// Also remove raw events older than this (≥ 1, else 400).
    pub older_than_days: Option<i64>,
}

/// Body of `POST /api/data/delete-events`. At least one of the two (else 400).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeleteIn {
    pub source: Option<String>,
    /// ISO-8601; without it the dry run fixes "until now" (app clock) and returns it in each
    /// removal's `before`, so the confirmation deletes exactly what was previewed.
    pub before: Option<String>,
}

// --- security ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityOut {
    pub token_file: String,
    /// When the token was created or last rotated (ISO-8601).
    pub token_changed: String,
    /// Names of the connected sources (they carry the token in their config).
    pub connected: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconnect {
    pub source: String,
    pub name: String,
    /// The diff shows `<new token>` in a dry run.
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotation {
    pub reconnects: Vec<Reconnect>,
    /// Connected sources that could not be reconnected (policy); applying is then refused (409).
    pub blocked: Vec<String>,
    pub applied: bool,
}

// --- stories and search (shapes from api/routes/stories.py) ---

/// `GET /api/stories/session/{id}`: `{session, title, source, connector, surface, models, project,
/// branches, started_at, ended_at, totals, items: [chapter | {kind: "markers", items}]}`.
pub type SessionStory = Json;
/// `GET /api/stories/project/{id}`: `{id, repo, name, git, system, totals, phases}`.
pub type ProjectStory = Json;
/// `GET /api/stories/prompt/{id}`: `{id, threshold, versions}`.
pub type Lineage = Json;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    /// Every match (all pages).
    pub total: u64,
    /// Newest first: `{id, received_at, session, title, project, branch, origin, outcome,
    /// cost_usd, facts, snippet: [{text, hit}]}`.
    pub hits: Vec<Json>,
}

// --- export ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Session,
    Project,
    Overview,
    Prompts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Md,
    #[default]
    Html,
    Csv,
    Json,
    Svg,
}

/// Export options as query parameters; each defaults to the most private choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportQuery {
    /// session: its id; project: its project id. Required for those kinds (else 422).
    pub id: Option<String>,
    pub format: ExportFormat,
    /// svg: the chart id; unknown or none = the first chart.
    pub chart: Option<String>,
    pub download: bool,
    pub prompt_text: bool,
    pub relative_paths: bool,
    pub drop_identity: bool,
    pub drop_raw: bool,
}

impl Default for ExportQuery {
    fn default() -> Self {
        ExportQuery {
            id: None,
            format: ExportFormat::Html,
            chart: None,
            download: false,
            prompt_text: false,
            relative_paths: true,
            drop_identity: true,
            drop_raw: true,
        }
    }
}

/// A rendered export. HTTP: `Content-Type: media_type`, `Content-Disposition: <inline|attachment>;
/// filename="<filename>"`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, and for
/// HTML `Content-Security-Policy: <csp>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportFile {
    pub media_type: String,
    /// `gitprompt-<kind>[-<title slug>]-<local day of the app clock>.<ext>`, ASCII.
    pub filename: String,
    pub attachment: bool,
    pub csp: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartInfo {
    pub id: String,
    pub title: String,
}
