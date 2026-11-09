//! The service facade: one typed async method per route, so the Tauri desktop app calls GitPrompt
//! directly (no HTTP, no token, no header) and gets exactly what the dashboard gets over HTTP.
//!
//! Every read brings the prompt index up to date first (a rules change rebuilds it), and every
//! call re-reads the source specs (cached, reloaded when a user spec file changes) and the
//! preferences, so edits apply without a restart. Blocking work (SQLite, git, config files) runs
//! off the async runtime. `Service` is cheap to clone and `Send + Sync`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use gitprompt_core::{Json, JsonMap, Settings};
use gitprompt_store::filter::PromptFilter;

use crate::bus::{Bus, Notice};
use crate::error::ApiResult;
use crate::model::{
    Annotation, Catalog, Change, ChartInfo, CompactIn, DataOut, DeleteIn, Enabled, ExportFile, ExportKind, ExportQuery, Lineage,
    ProjectStory, PromptDetail, PromptOut, PromptQuery, RecentCapture, ReportOut, Rotation, SearchResults, SecurityOut, SessionStory,
    SettingsOut, SettingsSaved, SourceOut, SpecProblemOut, SpecText, Stats, StatsQuery, Telemetry, Updated,
};
use crate::options::AppOptions;

/// The OTLP signal of `/ingest/{source}/v1/{signal}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Logs,
    Metrics,
}

#[derive(Clone)]
pub struct Service {
    _inner: Arc<()>,
}

impl Service {
    /// Open the store at `settings.db_path()` and create the receiver token if there is none.
    pub fn new(_settings: Settings, _options: AppOptions, _bus: Bus) -> ApiResult<Self> {
        todo!()
    }

    pub fn settings(&self) -> &Settings {
        todo!()
    }

    pub fn bus(&self) -> &Bus {
        todo!()
    }

    /// Bring the prompt index up to date (start-up does this before the first page).
    pub async fn ensure_index(&self) -> ApiResult<()> {
        todo!()
    }

    // --- ingest ---

    /// Capture one hook payload. An unknown source is `NotFound`; a source turned off in GitPrompt
    /// discards the payload (an empty notice, nothing published). Publishes the notice on the bus
    /// when something was stored.
    pub async fn ingest(&self, _source: &str, _payload: Json) -> ApiResult<Notice> {
        todo!()
    }

    /// Capture an OTLP/HTTP JSON export; `NotFound` for a source whose format is not OTLP.
    pub async fn ingest_otlp(&self, _source: &str, _signal: Signal, _payload: Json) -> ApiResult<Notice> {
        todo!()
    }

    // --- prompts ---

    /// Newest first, filtered on the prompt index.
    pub async fn prompts(&self, _query: PromptQuery) -> ApiResult<Vec<PromptOut>> {
        todo!()
    }

    pub async fn prompt(&self, _id: i64) -> ApiResult<PromptDetail> {
        todo!()
    }

    pub async fn annotate(&self, _id: i64, _annotation: Annotation) -> ApiResult<Annotation> {
        todo!()
    }

    /// Hide detected secrets in the prompt, its raw payload, its events and the index.
    pub async fn redact(&self, _id: i64) -> ApiResult<Updated> {
        todo!()
    }

    // --- sources ---

    pub async fn sources(&self) -> ApiResult<Vec<SourceOut>> {
        todo!()
    }

    pub async fn recent_captures(&self) -> ApiResult<Vec<RecentCapture>> {
        todo!()
    }

    pub async fn spec_problems(&self) -> ApiResult<Vec<SpecProblemOut>> {
        todo!()
    }

    pub async fn spec_text(&self, _source: &str) -> ApiResult<SpecText> {
        todo!()
    }

    /// Refused (`Conflict`) while the source is disabled in GitPrompt.
    pub async fn connect(&self, _source: &str, _dry_run: bool) -> ApiResult<Change> {
        todo!()
    }

    pub async fn disconnect(&self, _source: &str, _dry_run: bool) -> ApiResult<Change> {
        todo!()
    }

    pub async fn enable(&self, _source: &str) -> ApiResult<Enabled> {
        todo!()
    }

    pub async fn disable(&self, _source: &str) -> ApiResult<Enabled> {
        todo!()
    }

    /// Rebuild mapped fields from stored raw payloads (and the session join), then the index.
    pub async fn remap(&self, _source: &str) -> ApiResult<Updated> {
        todo!()
    }

    // --- stats and fields ---

    pub async fn stats(&self, _query: StatsQuery) -> ApiResult<Stats> {
        todo!()
    }

    pub async fn telemetry(&self, _filter: PromptFilter) -> ApiResult<Telemetry> {
        todo!()
    }

    pub async fn fields(&self) -> ApiResult<Catalog> {
        todo!()
    }

    // --- settings, data, security ---

    pub async fn preferences(&self) -> ApiResult<SettingsOut> {
        todo!()
    }

    pub async fn update_preferences(&self, _changes: JsonMap) -> ApiResult<SettingsSaved> {
        todo!()
    }

    pub async fn data(&self) -> ApiResult<DataOut> {
        todo!()
    }

    pub async fn compact(&self, _body: CompactIn, _dry_run: bool) -> ApiResult<ReportOut> {
        todo!()
    }

    pub async fn delete_events(&self, _body: DeleteIn, _dry_run: bool) -> ApiResult<ReportOut> {
        todo!()
    }

    pub async fn security(&self) -> ApiResult<SecurityOut> {
        todo!()
    }

    pub async fn rotate_token(&self, _dry_run: bool) -> ApiResult<Rotation> {
        todo!()
    }

    // --- stories, search, export ---

    pub async fn session_story(&self, _session: &str) -> ApiResult<SessionStory> {
        todo!()
    }

    pub async fn project_story(&self, _project: &str) -> ApiResult<ProjectStory> {
        todo!()
    }

    pub async fn lineage(&self, _id: i64) -> ApiResult<Lineage> {
        todo!()
    }

    /// Every word of `filter.q` in the typed text, newest first, with snippets; `total` ignores
    /// `limit` and `before`.
    pub async fn search(&self, _query: PromptQuery) -> ApiResult<SearchResults> {
        todo!()
    }

    /// `filter` applies to `overview` and `prompts`. An unknown chart id renders the first chart.
    pub async fn export(&self, _kind: ExportKind, _query: ExportQuery, _filter: PromptFilter) -> ApiResult<ExportFile> {
        todo!()
    }

    /// The charts of an export that have data.
    pub async fn export_charts(&self, _kind: ExportKind, _query: ExportQuery, _filter: PromptFilter) -> ApiResult<Vec<ChartInfo>> {
        todo!()
    }

    // --- housekeeping (the daemon's background tasks) ---

    /// One retention pass with the `event_retention_days` preference at the app clock's now.
    /// Returns the events deleted (0 when retention is off).
    pub async fn retention_pass(&self) -> ApiResult<u64> {
        todo!()
    }

    /// Ingest every payload the hook forwarder spooled while the daemon was down, oldest first,
    /// removing each one delivered. Returns how many were delivered.
    pub async fn drain_spool(&self) -> ApiResult<u64> {
        todo!()
    }
}
