//! The service facade: one typed async method per route, so the Tauri desktop app calls GitPrompt
//! directly (no HTTP, no token, no header) and gets exactly what the dashboard gets over HTTP.
//!
//! Every read brings the prompt index up to date first (a rules change rebuilds it), and every
//! call re-reads the source specs (cached, reloaded when a user spec file changes) and the
//! preferences, so edits apply without a restart. Blocking work (SQLite, git, config files) runs
//! off the async runtime. `Service` is cheap to clone and `Send + Sync`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use gitprompt_capture::connection::{self, Connection, StateFacts};
use gitprompt_capture::pipeline::{CaptureOptions, Captured, GitProbe, Observer, capture_then};
use gitprompt_capture::probe::{self, VersionProbe};
use gitprompt_capture::rotation;
use gitprompt_capture::spool::{Spool, drain_into_pipeline};
use gitprompt_core::git::Commit;
use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::spec::{Format, Spec, SpecCache, SpecSet, read_spec_file};
use gitprompt_core::{Error, Json, JsonMap, Settings, secrets};
use gitprompt_export::{Context, ExportError, Request, Zone, charts, html, naming};
use gitprompt_insights::derivations::{self, Derived};
use gitprompt_insights::facts::{FactRow, attach};
use gitprompt_insights::indexer::{Indexer, prompt_facts};
use gitprompt_insights::snippets::{WIDTH, highlight};
use gitprompt_insights::stories::{self, TurnTools, USER_ORIGINS};
use gitprompt_insights::{captures, catalog, surface};
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, GroupBy};
use gitprompt_store::{PromptView, Store, housekeeping};

use crate::bus::{Bus, Notice};
use crate::dashboard::{Dashboard, DashboardInfo};
use crate::error::{ApiError, ApiResult};
use crate::model::{
    Annotation, Backup, CapturedSurface, Catalog, Change, ChartInfo, CompactIn, DataOut, DeleteIn, Enabled, ExportFile, ExportKind,
    ExportQuery, HOURS, Injected, Lineage, ProjectStory, PromptDetail, PromptOut, PromptQuery, RecentCapture, ReportOut, Rotation,
    SearchResults, SecurityOut, SessionStory, SettingOut, SettingsOut, SettingsSaved, SourceEvents, SourceOut, SpecProblemOut, SpecText,
    Stats, StatsQuery, Telemetry, Updated,
};
use crate::options::{AppOptions, GitProvider, ToolVersionProbe};
use crate::query::iso;
use crate::status::{self, StatusOut};

/// The OTLP signal of `/ingest/{source}/v1/{signal}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Logs,
    Metrics,
}

/// Payloads read per spool drain and per field catalog (Python's `store.samples()` default).
const SAMPLES: usize = 5000;
/// Characters of a subagent brief in a session story (Python's `timeline.turn_events` default).
const BRIEF_CHARS: usize = 600;
/// Prompts compared when looking for the versions of one, newest first.
const LINEAGE_CANDIDATES: usize = 5000;

#[derive(Clone)]
pub struct Service {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service").finish_non_exhaustive()
    }
}

/// Everything a request needs, shared by every clone of the [`Service`].
struct Inner {
    settings: Settings,
    options: AppOptions,
    store: Store,
    /// Source specs, reloaded when a user spec file changes.
    specs: SpecCache,
    /// The indexer of the current spec set (rebuilt when the set is reloaded).
    indexer: Mutex<Option<(Arc<SpecSet>, Arc<Indexer>)>>,
    preferences: PreferenceFile,
    bus: Bus,
    /// Cancelled when the HTTP server shuts down: open live-update streams end.
    closing: CancellationToken,
    /// When this service was built (the app clock), for `/api/status`.
    started_at: jiff::Timestamp,
}

/// The app's git provider as the capture pipeline's probe.
struct GitAdapter<'a>(&'a dyn GitProvider);

impl GitProbe for GitAdapter<'_> {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        self.0.context(cwd)
    }
}

/// The app's tool-version probe as the capture pipeline's.
struct VersionAdapter<'a>(&'a dyn ToolVersionProbe);

impl VersionProbe for VersionAdapter<'_> {
    fn tool_version(&self, spec: &Spec) -> Option<String> {
        self.0.version(spec)
    }
}

impl Inner {
    fn spec_set(&self) -> Arc<SpecSet> {
        self.specs.get()
    }

    /// The spec of `source` in `set`; `NotFound` naming it.
    fn spec(set: &SpecSet, source: &str) -> ApiResult<Arc<Spec>> {
        set.get(source).cloned().ok_or_else(|| Error::NotFound(format!("unknown source '{source}'")).into())
    }

    /// The indexer of `set`, built once per spec set.
    fn indexer(&self, set: &Arc<SpecSet>) -> Arc<Indexer> {
        // The slot only ever holds a whole (set, indexer) pair, so a poisoned lock is still valid.
        let mut slot = self.indexer.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((cached, indexer)) = slot.as_ref()
            && Arc::ptr_eq(cached, set)
        {
            return Arc::clone(indexer);
        }
        let indexer = Arc::new(Indexer::new(set.specs.clone()));
        *slot = Some((Arc::clone(set), Arc::clone(&indexer)));
        indexer
    }

    /// The current specs and their indexer, with the prompt index brought up to date.
    fn indexed(&self) -> ApiResult<(Arc<SpecSet>, Arc<Indexer>)> {
        let set = self.spec_set();
        let indexer = self.indexer(&set);
        indexer.ensure(&self.store)?;
        Ok((set, indexer))
    }

    fn connection<'a>(&'a self, spec: &'a Spec) -> Connection<'a> {
        Connection::new(spec, &self.settings).via(self.options.forwarder.as_deref())
    }

    fn now(&self) -> jiff::Timestamp {
        self.options.clock.now()
    }

    /// Capture one request body for `spec` (marking the turns that got events for the index in the
    /// same transaction) and announce what was stored. The git and `--version` probes run before
    /// that transaction, so they never hold the writer (see `capture_then`).
    fn capture(&self, spec: &Spec, payload: &Json) -> ApiResult<Notice> {
        let options = CaptureOptions::for_source(&self.preferences.effective(), &spec.id);
        if !options.enabled {
            return Ok(Notice::default());
        }
        let (git, versions) = (GitAdapter(self.options.git.as_ref()), VersionAdapter(self.options.tool_version.as_ref()));
        let observer = Observer { git: &git, versions: &versions };
        let captured = capture_then(spec, &self.store, payload, options, &observer, None, mark)?;
        Ok(self.announce(&captured))
    }

    fn announce(&self, captured: &Captured) -> Notice {
        let notice = Notice::from(captured.notice());
        if !captured.is_empty() {
            self.bus.publish(notice);
        }
        notice
    }

    /// Commits in the story's repository while it lasted (one git call); `[]` without one.
    fn log(&self, rows: &[FactRow]) -> Vec<Commit> {
        let Some((since, until)) = stories::log_window(rows) else { return Vec::new() };
        let filled = |v: &Option<String>| v.as_deref().is_some_and(|v| !v.is_empty());
        let repo =
            rows.iter().find(|r| filled(&r.prompt.row.branch) && filled(&r.prompt.row.repo)).and_then(|r| r.prompt.row.repo.as_deref());
        repo.map_or_else(Vec::new, |repo| self.options.git.history(Path::new(repo), &since, &until))
    }

    fn timeline(&self, set: &SpecSet, filter: &PromptFilter, limit: Option<usize>, newest_first: bool) -> ApiResult<Vec<FactRow>> {
        Ok(attach(self.store.timeline(filter, limit, newest_first)?, &set.specs))
    }

    fn settings_out(&self) -> ApiResult<SettingsOut> {
        let settings = self
            .preferences
            .resolve()?
            .into_iter()
            .map(|r| {
                let out =
                    SettingOut { value: r.value, default: r.default, source: r.source, env: r.env, locked: r.locked, applies: r.applies };
                (r.key, out)
            })
            .collect();
        Ok(SettingsOut { file: self.preferences.path.display().to_string(), settings })
    }

    fn export(&self, kind: ExportKind, query: &ExportQuery, mut filter: PromptFilter) -> ApiResult<gitprompt_export::Export> {
        let (set, _) = self.indexed()?;
        filter.before = None; // an export covers every page
        let id = || query.id.clone().ok_or_else(|| ApiError::Unprocessable(format!("a {} export needs an id", kind_name(kind))));
        let request = match kind {
            ExportKind::Session => Request::Session { id: id()? },
            ExportKind::Project => Request::Project { id: id()? },
            ExportKind::Overview => Request::Overview { filter },
            ExportKind::Prompts => Request::Prompts { filter },
        };
        let context = Context {
            home: Some(self.settings.user_home.display().to_string()),
            roots: std::collections::BTreeSet::default(),
            zone: Zone::Local,
        };
        let generated_at = self.local_now().strftime("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string();
        gitprompt_export::build_from_store_with(&self.store, &set.specs, &request, &query.options(), &context, &generated_at)
            .map_err(export_error)
    }

    fn local_now(&self) -> jiff::Zoned {
        self.now().to_zoned(jiff::tz::TimeZone::system())
    }
}

/// Mark the turns that received events, so the index recomputes them on its next read.
fn mark(store: &Store, captured: &Captured) -> gitprompt_core::Result<()> {
    if captured.turns.is_empty() {
        return Ok(());
    }
    store.index().mark(&captured.turns.iter().cloned().collect::<Vec<_>>())
}

fn kind_name(kind: ExportKind) -> &'static str {
    gitprompt_export::Kind::from(kind).as_str()
}

/// No chart with data is a 404 like an unknown id; everything else keeps its domain status.
fn export_error(error: ExportError) -> ApiError {
    match error {
        ExportError::NoChart => ApiError::NotFound(error.to_string()),
        other => ApiError::Domain(other.into()),
    }
}

fn to_json(value: &impl Serialize) -> ApiResult<Json> {
    Ok(serde_json::to_value(value).map_err(Error::from)?)
}

fn to_map(value: &impl Serialize) -> ApiResult<JsonMap> {
    Ok(match to_json(value)? {
        Json::Object(map) => map,
        _ => JsonMap::new(),
    })
}

/// A prompt as the list shows it, from its derivations and facts.
fn prompt_out(derived: &Derived, spec: Option<&Spec>, outcome: Option<gitprompt_store::Outcome>) -> ApiResult<PromptOut> {
    let row = &derived.prompt.row;
    Ok(PromptOut {
        id: row.id,
        source: row.source.clone(),
        received_at: row.received_at.clone(),
        prompt: row.prompt.clone(),
        origin: derived.origin(),
        typed_text: derived.classified.typed_text.clone(),
        injected: derived.classified.injected.iter().map(|i| Injected { kind: i.kind.clone(), chars: i.chars as u64 }).collect(),
        fields: row.fields.clone(),
        context: row.context.clone(),
        annotation: derived.prompt.annotation.clone().into(),
        stats: to_map(&derived.stats)?,
        summary: to_map(&derived.summary)?,
        facts: prompt_facts(derived, spec, outcome).into(),
    })
}

/// The id a prompt's turn events carry.
fn turn_id(view: &PromptView) -> Option<String> {
    view.row.fields.get("prompt_id").and_then(Json::as_str).filter(|pid| !pid.is_empty()).map(str::to_owned)
}

/// [`HOURS`] as an `i64` (24 always fits).
#[expect(clippy::cast_possible_wrap, reason = "24 fits any integer type")]
const HOURS_I64: i64 = HOURS as i64;

/// Prompts plus events per UTC hour of the last [`HOURS`] hours, per source, the current hour last.
fn hourly(store: &Store, now: jiff::Timestamp) -> ApiResult<HashMap<String, Vec<u64>>> {
    let hour = now.as_second().div_euclid(3600) * 3600;
    let start = jiff::Timestamp::from_second(hour - (HOURS_I64 - 1) * 3600).map_err(|e| ApiError::Internal(e.to_string()))?;
    let slots: HashMap<String, usize> = (0..HOURS)
        .filter_map(|i| {
            Some((start.checked_add(jiff::SignedDuration::from_hours(i64::try_from(i).ok()?)).ok()?.strftime("%Y-%m-%dT%H").to_string(), i))
        })
        .collect();
    let mut out: HashMap<String, Vec<u64>> = HashMap::new();
    for count in store.hourly(&iso(start))? {
        if let Some(&slot) = slots.get(&count.hour)
            && let Some(total) = out.entry(count.source).or_insert_with(|| vec![0; HOURS]).get_mut(slot)
        {
            *total += count.rows;
        }
    }
    Ok(out)
}

/// A file's modification time, ISO-8601 UTC.
fn modified(meta: &std::fs::Metadata) -> String {
    meta.modified().ok().and_then(|t| jiff::Timestamp::try_from(t).ok()).map(iso).unwrap_or_default()
}

/// Config-file copies in `folder`, newest first.
fn backups(folder: &Path) -> Vec<Backup> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut files: Vec<(PathBuf, std::fs::Metadata)> =
        entries.filter_map(Result::ok).filter_map(|e| Some((e.path(), e.metadata().ok()?))).filter(|(_, m)| m.is_file()).collect();
    files.sort_by_key(|(_, m)| std::cmp::Reverse(m.modified().ok()));
    files
        .into_iter()
        .map(|(path, meta)| Backup {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.display().to_string(),
            bytes: meta.len(),
            modified: modified(&meta),
        })
        .collect()
}

impl Service {
    /// Open the store at `settings.db_path()` and create the receiver token if there is none.
    pub fn new(settings: Settings, options: AppOptions, bus: Bus) -> ApiResult<Self> {
        settings.token()?;
        let store = Store::open(&settings.db_path())?;
        let inner = Inner {
            specs: SpecCache::new(settings.clone()),
            preferences: PreferenceFile::new(&settings.home, settings.env.clone()),
            indexer: Mutex::new(None),
            closing: CancellationToken::new(),
            started_at: options.clock.now(),
            settings,
            options,
            store,
            bus,
        };
        Ok(Service { inner: Arc::new(inner) })
    }

    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    pub fn bus(&self) -> &Bus {
        &self.inner.bus
    }

    /// The built dashboard to serve, if any.
    pub(crate) fn dashboard(&self) -> Option<&Dashboard> {
        self.inner.options.dashboard.as_ref()
    }

    /// What the dashboard is served from (`AppOptions::dashboard`), and whether it is built.
    pub fn dashboard_info(&self) -> DashboardInfo {
        self.inner.options.dashboard.as_ref().map_or_else(DashboardInfo::none, Dashboard::info)
    }

    /// Cancelled when the HTTP server shuts down (open live-update streams end then).
    pub fn closing(&self) -> CancellationToken {
        self.inner.closing.clone()
    }

    /// Run `f` off the async runtime (SQLite, git and config files block).
    async fn run<T: Send + 'static>(&self, f: impl FnOnce(&Inner) -> ApiResult<T> + Send + 'static) -> ApiResult<T> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || f(&inner)).await.map_err(|e| ApiError::Internal(format!("background task failed: {e}")))?
    }

    /// Ask every tool its `--version` in the background (start-up), so neither the first capture
    /// nor the first connectors page waits for the commands. Fire and forget: a probe that fails
    /// is remembered as "no version" like any other. Plain detached threads, not `spawn_blocking`:
    /// the runtime waits for its blocking tasks when it shuts down, so quitting right after start
    /// would wait for a slow `--version` (up to its timeout).
    pub fn warm_tool_versions(&self) {
        for spec in self.inner.spec_set().specs.values().filter(|spec| !spec.version_cmd.is_empty()) {
            let (probe, spec) = (Arc::clone(&self.inner.options.tool_version), Arc::clone(spec));
            let started = std::thread::Builder::new().name("gitprompt-version".into()).spawn(move || probe.version(&spec));
            if let Err(e) = started {
                tracing::debug!(error = %e, "could not warm a tool version");
            }
        }
    }

    /// Bring the prompt index up to date (start-up does this before the first page).
    /// Unlike the update before each read, this always runs the update transaction once
    /// (`Indexer::catch_up`), which moves the telemetry watermark to the newest event.
    pub async fn ensure_index(&self) -> ApiResult<()> {
        self.run(|inner| {
            let set = inner.spec_set();
            inner.indexer(&set).catch_up(&inner.store)?;
            Ok(())
        })
        .await
    }

    // --- ingest ---

    /// Capture one hook payload. An unknown source is `NotFound`; a source turned off in GitPrompt
    /// discards the payload (an empty notice, nothing published). Publishes the notice on the bus
    /// when something was stored.
    pub async fn ingest(&self, source: &str, payload: Json) -> ApiResult<Notice> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            inner.capture(&spec, &payload)
        })
        .await
    }

    /// Capture an OTLP/HTTP JSON export; `NotFound` for a source whose format is not OTLP.
    pub async fn ingest_otlp(&self, source: &str, _signal: Signal, payload: Json) -> ApiResult<Notice> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            if spec.format != Format::Otlp {
                return Err(Error::NotFound(format!("source '{}' does not receive OpenTelemetry exports", spec.id)).into());
            }
            inner.capture(&spec, &payload)
        })
        .await
    }

    // --- prompts ---

    /// Newest first, filtered on the prompt index.
    pub async fn prompts(&self, query: PromptQuery) -> ApiResult<Vec<PromptOut>> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let rows = store.search(&query.filter, query.limit)?;
            let turns: Vec<String> = rows.iter().filter_map(turn_id).collect();
            let mut events: HashMap<String, Vec<_>> = HashMap::new();
            for event in store.events_for_prompts(&turns)? {
                if let Some(pid) = event.prompt_id.clone() {
                    events.entry(pid).or_default().push(event);
                }
            }
            let outcomes = store.outcomes(&rows.iter().map(|r| r.row.id).collect::<Vec<_>>())?;
            rows.iter()
                .map(|row| {
                    let spec = set.get(&row.row.source).map(AsRef::as_ref);
                    let turn = turn_id(row).and_then(|pid| events.get(&pid)).map_or(&[][..], Vec::as_slice);
                    prompt_out(&derivations::derive(row, turn, spec), spec, outcomes.get(&row.row.id).copied())
                })
                .collect()
        })
        .await
    }

    pub async fn prompt(&self, id: i64) -> ApiResult<PromptDetail> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let row = store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let events = match turn_id(&row) {
                Some(pid) => store.events_for_prompts(&[pid])?,
                None => Vec::new(),
            };
            let spec = set.get(&row.row.source).map(AsRef::as_ref);
            let outcome = store.outcomes(&[id])?.get(&id).copied();
            let prompt = prompt_out(&derivations::derive(&row, &events, spec), spec, outcome)?;
            let session = match row.row.fields.get("session_id").and_then(Json::as_str) {
                Some(sid) if !sid.is_empty() => to_map(&derivations::session(&store.events_for_session(sid)?))?,
                _ => JsonMap::new(),
            };
            let commits = commits_after(inner, &row)?;
            Ok(PromptDetail { prompt, raw: row.row.raw.clone(), session, commits, events })
        })
        .await
    }

    pub async fn annotate(&self, id: i64, annotation: Annotation) -> ApiResult<Annotation> {
        if !(-1..=1).contains(&annotation.rating) {
            return Err(ApiError::Unprocessable(format!("rating must be -1, 0 or 1, got {}", annotation.rating)));
        }
        self.run(move |inner| {
            inner.store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            inner.store.annotate(id, &annotation.clone().into())?;
            Ok(annotation)
        })
        .await
    }

    /// Hide detected secrets in the prompt, its raw payload, its events and the index.
    pub async fn redact(&self, id: i64) -> ApiResult<Updated> {
        self.run(move |inner| {
            let (_, indexer) = inner.indexed()?;
            inner.store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let updated = inner.store.rewrite(id, &secrets::redact)?;
            indexer.reindex(&inner.store, &[id])?;
            Ok(Updated { updated })
        })
        .await
    }

    // --- sources ---

    pub async fn sources(&self) -> ApiResult<Vec<SourceOut>> {
        self.run(|inner| {
            let set = inner.spec_set();
            let stats = inner.store.source_stats()?;
            let activity = hourly(&inner.store, inner.now())?;
            let preferences = inner.preferences.effective();
            let specs: Vec<&Spec> = set.specs.values().map(AsRef::as_ref).collect();
            // A tool's `--version` may take a while: ask every tool at once.
            let versions: Vec<Option<String>> = std::thread::scope(|scope| {
                let probe = inner.options.tool_version.as_ref();
                #[expect(
                    clippy::needless_collect,
                    reason = "spawn every probe before joining any: collecting is what runs them in parallel"
                )]
                let running: Vec<_> = specs.iter().map(|spec| scope.spawn(move || probe.version(spec))).collect();
                running.into_iter().map(|h| h.join().unwrap_or(None)).collect()
            });
            specs
                .into_iter()
                .zip(versions)
                .map(|(spec, version)| {
                    let connection = inner.connection(spec);
                    let source_stats = stats.get(&spec.id);
                    let facts = StateFacts::gather_for(&connection, &preferences, source_stats);
                    let missing = probe::missing_capabilities(spec, version.as_deref());
                    let problem = set.problems.iter().find(|p| p.fallback.as_deref() == Some(spec.id.as_str()));
                    let file =
                        set.files.get(&spec.id).cloned().ok_or_else(|| ApiError::Internal(format!("no spec file for {}", spec.id)))?;
                    Ok(SourceOut {
                        id: spec.id.clone(),
                        name: spec.name.clone(),
                        assistant: spec.assistant.clone(),
                        part_of: spec.part_of.clone(),
                        layer: spec.layer.clone(),
                        summary: spec.summary.clone(),
                        enabled: facts.enabled,
                        detected: facts.detected,
                        version,
                        connected: facts.connected,
                        outdated: facts.outdated,
                        config_file: connection.config_file().display().to_string(),
                        terms: spec.terms.clone(),
                        after_connect: spec.after_connect.clone(),
                        blocks: facts.blocks,
                        warnings: facts.warnings,
                        total: source_stats.map_or(0, |s| s.total),
                        unmapped: source_stats.map_or(0, |s| s.unmapped),
                        events: source_stats.map_or(0, |s| s.events),
                        last_at: source_stats.and_then(|s| s.last_at.clone()),
                        hourly: activity.get(&spec.id).cloned().unwrap_or_else(|| vec![0; HOURS]),
                        collects: spec.documented.keys().filter(|e| *e != "*" && spec.keeps(Some(e))).cloned().collect(),
                        not_stored: spec.drop_events.clone(),
                        not_collected: spec.not_captured.clone(),
                        capabilities: spec.capabilities.iter().filter(|c| !missing.contains_key(*c)).copied().collect(),
                        needs_version: missing,
                        join: spec.join,
                        captures: spec
                            .captures
                            .iter()
                            .filter_map(|(surface, note)| {
                                let info = spec.surface_info(Some(surface))?;
                                Some(CapturedSurface { id: info.id, name: info.name, icon: info.icon, note: note.clone() })
                            })
                            .collect(),
                        cannot_capture: spec.cannot_capture.clone(),
                        spec: file.into(),
                        spec_problem: problem.cloned().map(Into::into),
                    })
                })
                .collect()
        })
        .await
    }

    pub async fn recent_captures(&self) -> ApiResult<Vec<RecentCapture>> {
        self.run(|inner| {
            let (set, _) = inner.indexed()?;
            Ok(captures::recent(&inner.store, &set.specs)?.into_iter().map(Into::into).collect())
        })
        .await
    }

    pub async fn spec_problems(&self) -> ApiResult<Vec<SpecProblemOut>> {
        self.run(|inner| Ok(inner.spec_set().problems.iter().cloned().map(Into::into).collect())).await
    }

    pub async fn spec_text(&self, source: &str) -> ApiResult<SpecText> {
        let source = source.to_owned();
        self.run(move |inner| {
            let set = inner.spec_set();
            let spec = Inner::spec(&set, &source)?;
            let file = set.files.get(&spec.id).cloned().ok_or_else(|| ApiError::Internal(format!("no spec file for {}", spec.id)))?;
            let text = read_spec_file(&file)?;
            Ok(SpecText { file: file.into(), text })
        })
        .await
    }

    /// Refused (`Conflict`) while the source is disabled in GitPrompt.
    pub async fn connect(&self, source: &str, dry_run: bool) -> ApiResult<Change> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            connection::refuse_if_disabled(&spec, &inner.preferences.effective())?;
            Ok(inner.connection(&spec).connect(dry_run)?.into())
        })
        .await
    }

    pub async fn disconnect(&self, source: &str, dry_run: bool) -> ApiResult<Change> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            Ok(inner.connection(&spec).disconnect(dry_run)?.into())
        })
        .await
    }

    pub async fn enable(&self, source: &str) -> ApiResult<Enabled> {
        self.set_enabled(source, true).await
    }

    pub async fn disable(&self, source: &str) -> ApiResult<Enabled> {
        self.set_enabled(source, false).await
    }

    async fn set_enabled(&self, source: &str, enabled: bool) -> ApiResult<Enabled> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            Ok(Enabled { enabled: connection::set_enabled(&spec, &inner.preferences, enabled)? })
        })
        .await
    }

    /// Rebuild mapped fields from stored raw payloads (and the session join), then the index.
    pub async fn remap(&self, source: &str) -> ApiResult<Updated> {
        let source = source.to_owned();
        self.run(move |inner| {
            let (set, indexer) = inner.indexed()?;
            let spec = Inner::spec(&set, &source)?;
            let updated = connection::remap(&spec, &inner.store)?;
            indexer.rebuild(&inner.store)?;
            Ok(Updated { updated })
        })
        .await
    }

    // --- stats and fields ---

    pub async fn stats(&self, query: StatsQuery) -> ApiResult<Stats> {
        if query.split_by.is_some() && query.group_by.is_none() {
            return Err(ApiError::Unprocessable("split_by needs group_by".into()));
        }
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let aggregate = AggregateQuery {
                filter: query.filter,
                group_by: query.group_by,
                split_by: query.split_by,
                order: query.order,
                limit: query.limit,
            };
            let mut result = inner.store.aggregate(&aggregate).map_err(|e| match e {
                Error::Invalid(message) => ApiError::Unprocessable(message),
                other => other.into(),
            })?;
            let surfaces = surface::catalog(set.specs.values().map(AsRef::as_ref));
            let name = |grouping: Option<GroupBy>, bucket: &mut gitprompt_store::stats::Bucket| match grouping {
                Some(GroupBy::Source) => {
                    if let Some(spec) = bucket.key.as_deref().and_then(|k| set.get(k))
                        && bucket.detail.is_none()
                    {
                        bucket.detail = Some(spec.label().to_owned());
                    }
                }
                Some(GroupBy::Surface) => {
                    if let Some(info) = surfaces.get(bucket.key.as_deref().unwrap_or_default()) {
                        bucket.detail = Some(info.name.clone());
                    }
                }
                _ => {}
            };
            for group in &mut result.groups {
                name(query.group_by, group);
                for part in group.parts.iter_mut().flatten() {
                    name(query.split_by, part);
                }
            }
            Ok(Stats { group_by: query.group_by, split_by: query.split_by, aggregate: result })
        })
        .await
    }

    pub async fn telemetry(&self, filter: PromptFilter) -> ApiResult<Telemetry> {
        self.run(move |inner| {
            inner.indexed()?;
            Ok(inner.store.telemetry(&filter)?)
        })
        .await
    }

    pub async fn fields(&self) -> ApiResult<Catalog> {
        self.run(|inner| {
            let set = inner.spec_set();
            to_json(&catalog::build_catalog(&inner.store.samples(SAMPLES)?, set.specs.values().map(AsRef::as_ref)))
        })
        .await
    }

    // --- settings, data, security ---

    pub async fn preferences(&self) -> ApiResult<SettingsOut> {
        self.run(Inner::settings_out).await
    }

    pub async fn update_preferences(&self, changes: JsonMap) -> ApiResult<SettingsSaved> {
        self.run(move |inner| {
            let changed = inner.preferences.update(&changes)?;
            let restart_required = changed.iter().any(|key| inner.options.applies_at_start.contains(key));
            Ok(SettingsSaved { current: inner.settings_out()?, changed, restart_required })
        })
        .await
    }

    pub async fn data(&self) -> ApiResult<DataOut> {
        self.run(|inner| {
            let mut events: Vec<SourceEvents> = Vec::new();
            for usage in inner.store.event_usage(None)? {
                match events.iter_mut().find(|s| s.source == usage.source) {
                    Some(s) => {
                        s.rows += usage.rows;
                        s.bytes += usage.bytes;
                    }
                    None => events.push(SourceEvents { source: usage.source, rows: usage.rows, bytes: usage.bytes }),
                }
            }
            events.sort_by_key(|s| std::cmp::Reverse(s.bytes));
            let settings = &inner.settings;
            Ok(DataOut {
                home: settings.home.display().to_string(),
                db_path: settings.db_path().display().to_string(),
                db_bytes: inner.store.size()?,
                receiver: settings.receiver(),
                events,
                backups: backups(&settings.backups()),
            })
        })
        .await
    }

    pub async fn compact(&self, body: CompactIn, dry_run: bool) -> ApiResult<ReportOut> {
        if body.older_than_days.is_some_and(|days| days < 1) {
            return Err(Error::Invalid("older_than_days must be at least 1".into()).into());
        }
        self.run(move |inner| {
            let set = inner.spec_set();
            Ok(housekeeping::compact(&inner.store, &set.specs, body.older_than_days, inner.now(), !dry_run)?.into())
        })
        .await
    }

    pub async fn delete_events(&self, body: DeleteIn, dry_run: bool) -> ApiResult<ReportOut> {
        if body.source.is_none() && body.before.is_none() {
            return Err(Error::Invalid("choose a source or a date: deleting every event is not offered".into()).into());
        }
        let before = body.before.as_deref().map(|b| crate::query::instant("before", b)).transpose()?;
        self.run(move |inner| {
            let before = before.unwrap_or_else(|| iso(inner.now()));
            let removals = housekeeping::selected(&inner.store, body.source.as_deref(), Some(&before))?;
            Ok(housekeeping::plan(&inner.store, removals, !dry_run)?.into())
        })
        .await
    }

    pub async fn security(&self) -> ApiResult<SecurityOut> {
        self.run(|inner| {
            inner.settings.token()?;
            let token_file = inner.settings.token_file();
            let token_changed = modified(&std::fs::metadata(&token_file).map_err(Error::from)?);
            let set = inner.spec_set();
            let connected = set.specs.values().filter(|spec| inner.connection(spec).connected()).map(|spec| spec.name.clone()).collect();
            Ok(SecurityOut { token_file: token_file.display().to_string(), token_changed, connected })
        })
        .await
    }

    pub async fn rotate_token(&self, dry_run: bool) -> ApiResult<Rotation> {
        self.run(move |inner| {
            let set = inner.spec_set();
            Ok(rotation::rotate_via(&set.specs, &inner.settings, dry_run, inner.options.forwarder.as_deref())?.into())
        })
        .await
    }

    // --- stories, search, export ---

    pub async fn session_story(&self, session: &str) -> ApiResult<SessionStory> {
        let session = session.to_owned();
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let filter = PromptFilter { session: vec![session.clone()], ..PromptFilter::default() };
            let rows = inner.timeline(&set, &filter, None, false)?;
            let first = rows.first().ok_or_else(|| Error::NotFound("session not found".into()))?;
            let user: Vec<FactRow> = rows.iter().filter(|r| stories::is_user(r)).cloned().collect();
            let turns: Vec<String> = rows.iter().filter_map(|r| r.prompt.row.prompt_id.clone()).collect();
            let events = store.turn_events(&turns, BRIEF_CHARS)?;
            let commits = inner.log(&user);
            let tool_kinds = store.tool_kinds(&user.iter().map(|r| r.prompt.row.id).collect::<Vec<_>>())?;
            let spec = set.get(&first.prompt.row.source);
            let (none_kinds, none_files) = Default::default();
            let tools = TurnTools {
                tool_kinds: &tool_kinds,
                events: &events,
                kinds: spec.map_or(&none_kinds, |s| &s.tool_kinds),
                tool_files: spec.map_or(&none_files, |s| &s.tool_files),
            };
            to_json(&stories::session_story(&session, &rows, tools, &commits))
        })
        .await
    }

    pub async fn project_story(&self, project: &str) -> ApiResult<ProjectStory> {
        let project = project.to_owned();
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let not_found = || ApiError::from(Error::NotFound("project not found".into()));
            let repo = stories::project_repo(&project).ok_or_else(not_found)?;
            let every = PromptFilter { repo: Some(repo.clone()), ..PromptFilter::default() };
            let typed = PromptFilter { origin: USER_ORIGINS.to_vec(), ..every.clone() };
            let rows = inner.timeline(&set, &typed, None, false)?;
            let total = inner.store.count(&every)?;
            if rows.is_empty() && total == 0 {
                return Err(not_found());
            }
            let system = total - i64::try_from(rows.len()).unwrap_or(i64::MAX);
            let commits = inner.log(&rows);
            let tool_kinds = inner.store.tool_kinds(&rows.iter().map(|r| r.prompt.row.id).collect::<Vec<_>>())?;
            to_json(&stories::project_story(&repo, &rows, &tool_kinds, &commits, system))
        })
        .await
    }

    pub async fn lineage(&self, id: i64) -> ApiResult<Lineage> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let target = inner.store.timeline_one(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let target = attach(vec![target], &set.specs).pop().ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let filter = PromptFilter { origin: USER_ORIGINS.to_vec(), ..PromptFilter::default() };
            let candidates = inner.timeline(&set, &filter, Some(LINEAGE_CANDIDATES), true)?;
            to_json(&stories::prompt_lineage(&target, &candidates))
        })
        .await
    }

    /// Every word of `filter.q` in the typed text, newest first, with snippets; `total` ignores
    /// `limit` and `before`.
    pub async fn search(&self, query: PromptQuery) -> ApiResult<SearchResults> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let rows = inner.timeline(&set, &query.filter, Some(query.limit), true)?;
            let total = inner.store.count(&PromptFilter { before: None, ..query.filter.clone() })?;
            let hits = rows
                .iter()
                .map(|r| {
                    let row = &r.prompt.row;
                    json!({
                        "id": row.id,
                        "received_at": row.received_at,
                        "session": row.session,
                        "title": row.title,
                        "project": stories::project_of(std::slice::from_ref(r)),
                        "branch": row.branch,
                        "origin": row.origin,
                        "outcome": r.prompt.outcome,
                        "cost_usd": row.usage.cost_usd,
                        "facts": r.facts,
                        "snippet": highlight(row.typed_text.as_deref(), query.filter.q.as_deref(), WIDTH),
                    })
                })
                .collect();
            Ok(SearchResults { total: u64::try_from(total).unwrap_or(0), hits })
        })
        .await
    }

    /// `filter` applies to `overview` and `prompts`. An unknown chart id renders the first chart.
    pub async fn export(&self, kind: ExportKind, query: ExportQuery, filter: PromptFilter) -> ApiResult<ExportFile> {
        self.run(move |inner| {
            let export = inner.export(kind, &query, filter)?;
            let format = gitprompt_export::Format::from(query.format);
            let body = gitprompt_export::render(&export, format, query.chart.as_deref()).map_err(export_error)?;
            Ok(ExportFile {
                media_type: format.media_type().to_owned(),
                filename: naming::filename(&export.report, format, inner.local_now().date()),
                attachment: query.download,
                csp: (format == gitprompt_export::Format::Html).then(|| html::CSP.to_owned()),
                body,
            })
        })
        .await
    }

    /// The charts of an export that have data.
    pub async fn export_charts(&self, kind: ExportKind, query: ExportQuery, filter: PromptFilter) -> ApiResult<Vec<ChartInfo>> {
        self.run(move |inner| {
            let export = inner.export(kind, &query, filter)?;
            Ok(charts::chart_list(&export.report).into_iter().map(|c| ChartInfo { id: c.id, title: c.title }).collect())
        })
        .await
    }

    // --- status ---

    /// Version, uptime, database size and counts, index state, dashboard source and spool
    /// backlog (`GET /api/status`). Read-only: unlike the other reads it does not bring the index
    /// up to date first, so it reports what the next read will do.
    pub async fn status(&self) -> ApiResult<StatusOut> {
        let dashboard = self.dashboard_info();
        self.run(move |inner| {
            let set = inner.spec_set();
            let indexer = inner.indexer(&set);
            let counts = status::counts(&inner.store)?;
            let now = inner.now();
            let settings = &inner.settings;
            Ok(StatusOut {
                version: status::VERSION.to_owned(),
                backend: status::BACKEND.to_owned(),
                started_at: iso(inner.started_at),
                uptime_s: u64::try_from(now.duration_since(inner.started_at).as_secs()).unwrap_or(0),
                home: settings.home.display().to_string(),
                db_path: settings.db_path().display().to_string(),
                db_bytes: inner.store.size()?,
                prompts: counts.prompts,
                events: counts.events,
                index: status::index_status(&inner.store, indexer.version())?,
                dashboard,
                spool: status::spool_backlog(settings)?,
            })
        })
        .await
    }

    // --- housekeeping (the daemon's background tasks) ---

    /// One retention pass with the `event_retention_days` preference at the app clock's now.
    /// Returns the events deleted (0 when retention is off).
    pub async fn retention_pass(&self) -> ApiResult<u64> {
        self.run(|inner| Ok(housekeeping::retention_pass(&inner.store, &inner.preferences, inner.now())?)).await
    }

    /// Ingest every payload the hook forwarder spooled while the daemon was down, oldest first,
    /// removing each one delivered. Returns how many were delivered.
    pub async fn drain_spool(&self) -> ApiResult<u64> {
        self.run(|inner| {
            let set = inner.spec_set();
            let (git, versions) = (GitAdapter(inner.options.git.as_ref()), VersionAdapter(inner.options.tool_version.as_ref()));
            let observer = Observer { git: &git, versions: &versions };
            let spool = Spool::of(&inner.settings);
            let (report, captured) = drain_into_pipeline(&spool, &set.specs, &inner.store, &inner.preferences.effective(), &observer)?;
            mark(&inner.store, &captured)?;
            inner.announce(&captured);
            Ok(report.values().map(|d| d.ingested as u64).sum())
        })
        .await
    }
}

/// Commits made in the prompt's checkout until the next prompt of the session (or now); `{}`
/// outside a repository.
fn commits_after(inner: &Inner, row: &PromptView) -> ApiResult<JsonMap> {
    let context = &row.row.context;
    let text = |key: &str| context.get(key).and_then(Json::as_str).filter(|v| !v.is_empty());
    let Some(root) = text("git_root") else { return Ok(JsonMap::new()) };
    let checkout = text("git_worktree_root").unwrap_or(root); // a worktree has its own HEAD
    let end = row.next_head.as_deref().unwrap_or("HEAD");
    to_map(&inner.options.git.commits(Path::new(checkout), text("git_head"), end, &row.row.received_at))
}
