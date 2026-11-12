//! Wire types: what each route answers and the Service returns. Field names and optionality are the
//! wire contract with the dashboard, whose TypeScript types are generated from these
//! (`web/src/lib/api/generated.ts`, `just types`; `tests/ts_types.rs` fails while it is stale).
//!
//! Shapes computed by the insights crate (prompt stats and turn summary, session derivations, the
//! field catalog, stories, lineage) are re-exported from there, so they are typed end to end.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::CommitsAfter;
use gitprompt_core::preferences::{Applies, ValueSource};
use gitprompt_core::spec::{Capability, Join, Origin, SpecOrigin, SurfaceRef};
use gitprompt_core::{Json, JsonMap};
use gitprompt_insights::derivations::{PromptStats, SessionSummary, TurnSummary};
use gitprompt_insights::snippets::Segment;
use gitprompt_insights::stories::ProjectRef as StoryProject;
use gitprompt_store::filter::{Outcome, PromptFilter};
use gitprompt_store::housekeeping::Why;
use gitprompt_store::rows::EventRow;
use gitprompt_store::stats::{Aggregate, GroupBy, Order};

// --- prompts ---

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

/// What every page shows about a turn (insights facts, ADR-0021), and its parts.
pub use gitprompt_insights::facts::{Connector, Facts, ModelSource, Tokens};

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

// --- sources ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "SpecFile")]
pub struct SpecFileOut {
    pub name: String,
    pub path: String,
    pub origin: SpecOrigin,
    pub overrides: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "SpecProblem")]
pub struct SpecProblemOut {
    pub file: SpecFileOut,
    pub error: String,
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "Source")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CaptureProject")]
pub struct ProjectRef {
    pub repo: String,
    pub name: String,
}

/// The newest prompt of one connector and surface (`GET /api/sources/recent`), newest first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecText {
    pub file: SpecFileOut,
    pub text: String,
}

/// A connect/disconnect (or its dry run): the unified diff of the tool's config file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Change {
    pub file: String,
    /// Empty when there is nothing to change.
    pub diff: String,
    pub applied: bool,
    pub backup: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Stats {
    pub group_by: Option<GroupBy>,
    pub split_by: Option<GroupBy>,
    #[serde(flatten)]
    pub aggregate: Aggregate,
}

pub use gitprompt_store::stats::Telemetry;

// --- fields ---

/// `GET /api/fields`: every captured field next to what the tool documents.
pub use gitprompt_insights::catalog::Catalog;

// --- settings ---

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SettingOut {
    pub value: Json,
    pub default: Json,
    pub source: ValueSource,
    pub env: String,
    pub locked: bool,
    pub applies: Applies,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "SettingsState")]
pub struct SettingsOut {
    /// `<home>/preferences.json`.
    pub file: String,
    /// Every preference key, in `KEYS` order.
    pub settings: IndexMap<String, SettingOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SettingsSaved {
    #[serde(flatten)]
    pub current: SettingsOut,
    pub changed: Vec<String>,
    pub restart_required: bool,
}

// --- data ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Backup {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SourceEvents {
    pub source: String,
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "DataInfo")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Removal")]
pub struct RemovalOut {
    pub source: String,
    pub event: Option<String>,
    pub rows: u64,
    pub bytes: u64,
    pub why: Why,
    pub before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CleanupReport")]
pub struct ReportOut {
    pub removals: Vec<RemovalOut>,
    pub rows: u64,
    pub bytes: u64,
    pub size_before: u64,
    pub size_after: Option<u64>,
    pub applied: bool,
}

/// Body of `POST /api/data/compact`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct CompactIn {
    /// Also remove raw events older than this (≥ 1, else 400).
    pub older_than_days: Option<i64>,
}

/// Body of `POST /api/data/delete-events`. At least one of the two (else 400).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct DeleteIn {
    pub source: Option<String>,
    /// ISO-8601; without it the dry run fixes "until now" (app clock) and returns it in each
    /// removal's `before`, so the confirmation deletes exactly what was previewed.
    pub before: Option<String>,
}

// --- security ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Security")]
pub struct SecurityOut {
    pub token_file: String,
    /// When the token was created or last rotated (ISO-8601).
    pub token_changed: String,
    /// Names of the connected sources (they carry the token in their config).
    pub connected: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Reconnect {
    pub source: String,
    pub name: String,
    /// The diff shows `<new token>` in a dry run.
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Rotation {
    pub reconnects: Vec<Reconnect>,
    /// Connected sources that could not be reconnected (policy); applying is then refused (409).
    pub blocked: Vec<String>,
    pub applied: bool,
}

// --- stories and search ---

/// `GET /api/stories/prompt/{id}`: the prompt's retries and reuses.
pub use gitprompt_insights::stories::PromptLineage as Lineage;
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

// --- export ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Session,
    Project,
    Overview,
    Prompts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Md,
    #[default]
    Html,
    Csv,
    Json,
    Svg,
    /// One chart as a PNG, rendered natively (2× the SVG size).
    Png,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ChartInfo {
    pub id: String,
    pub title: String,
}

// --- conversions from the crates that compute these values (same wire shapes) ---

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

impl From<gitprompt_core::spec::SpecFile> for SpecFileOut {
    fn from(f: gitprompt_core::spec::SpecFile) -> Self {
        SpecFileOut { name: f.name, path: f.path, origin: f.origin, overrides: f.overrides }
    }
}

impl From<gitprompt_core::spec::SpecProblem> for SpecProblemOut {
    fn from(p: gitprompt_core::spec::SpecProblem) -> Self {
        SpecProblemOut { file: p.file.into(), error: p.error, fallback: p.fallback }
    }
}

impl From<gitprompt_capture::Change> for Change {
    fn from(c: gitprompt_capture::Change) -> Self {
        Change { file: c.file, diff: c.diff, applied: c.applied, backup: c.backup }
    }
}

impl From<gitprompt_capture::rotation::Rotation> for Rotation {
    fn from(r: gitprompt_capture::rotation::Rotation) -> Self {
        Rotation {
            reconnects: r.reconnects.into_iter().map(|c| Reconnect { source: c.source, name: c.name, change: c.change.into() }).collect(),
            blocked: r.blocked,
            applied: r.applied,
        }
    }
}

impl From<gitprompt_store::housekeeping::Report> for ReportOut {
    fn from(r: gitprompt_store::housekeeping::Report) -> Self {
        let (rows, bytes) = (r.rows(), r.bytes());
        let removals = r
            .removals
            .into_iter()
            .map(|m| RemovalOut { source: m.source, event: m.event, rows: m.rows, bytes: m.bytes, why: m.why, before: m.before })
            .collect();
        ReportOut { removals, rows, bytes, size_before: r.size_before, applied: r.size_after.is_some(), size_after: r.size_after }
    }
}

impl From<gitprompt_insights::captures::RecentCapture> for RecentCapture {
    fn from(r: gitprompt_insights::captures::RecentCapture) -> Self {
        RecentCapture {
            id: r.id,
            connector: r.connector,
            surface: r.surface,
            received_at: r.received_at,
            session: r.session,
            title: r.title,
            project: r.project.map(|p| ProjectRef { repo: p.repo, name: p.name }),
            branch: r.branch,
            worktree: r.worktree,
            model: r.model,
            origin: gitprompt_insights::origin::parse(&r.origin).unwrap_or(Origin::Unknown),
            prompts: r.prompts,
            signals: r.signals.into_iter().collect(),
        }
    }
}

impl From<ExportKind> for gitprompt_export::Kind {
    fn from(kind: ExportKind) -> Self {
        match kind {
            ExportKind::Session => gitprompt_export::Kind::Session,
            ExportKind::Project => gitprompt_export::Kind::Project,
            ExportKind::Overview => gitprompt_export::Kind::Overview,
            ExportKind::Prompts => gitprompt_export::Kind::Prompts,
        }
    }
}

impl From<ExportFormat> for gitprompt_export::Format {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::Md => gitprompt_export::Format::Markdown,
            ExportFormat::Html => gitprompt_export::Format::Html,
            ExportFormat::Csv => gitprompt_export::Format::Csv,
            ExportFormat::Json => gitprompt_export::Format::Json,
            ExportFormat::Svg => gitprompt_export::Format::Svg,
            ExportFormat::Png => gitprompt_export::Format::Png,
        }
    }
}

impl ExportQuery {
    /// The redaction options of the export.
    pub fn options(&self) -> gitprompt_export::Options {
        gitprompt_export::Options {
            prompt_text: self.prompt_text,
            relative_paths: self.relative_paths,
            drop_identity: self.drop_identity,
            drop_raw: self.drop_raw,
        }
    }
}
