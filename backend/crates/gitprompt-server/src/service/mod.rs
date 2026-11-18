//! The service facade: one typed async method per route, so the Tauri desktop app calls GitPrompt
//! directly (no HTTP, no token, no header) and gets exactly what the dashboard gets over HTTP.
//!
//! Every read brings the prompt index up to date first (a rules change rebuilds it), and every
//! call re-reads the source specs (cached, reloaded when a user spec file changes) and the
//! preferences, so edits apply without a restart. Blocking work (SQLite, git, config files) runs
//! off the async runtime. `Service` is cheap to clone and `Send + Sync`.
//!
//! This file holds the shared state and the service's lifecycle; each group of routes is one
//! submodule adding its methods to [`Service`].

mod commits;
mod data;
mod export;
mod find;
mod homes;
mod ingest;
mod names;
mod prompts;
mod resume;
mod settings;
mod sources;
mod stats;
mod stories;
mod usage;

use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use gitprompt_capture::connection::Connection;
use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::spec::{Spec, SpecCache, SpecSet};
use gitprompt_core::{Error, Settings};
use gitprompt_insights::indexer::Indexer;
use gitprompt_store::Store;

use crate::bus::Bus;
use crate::dashboard::{Dashboard, DashboardInfo};
use crate::error::{ApiError, ApiResult};
use crate::health::{Health, Monitor, ProcessSampler, Sample};
use crate::options::AppOptions;
use crate::query::iso;
use crate::status::{self, StatusOut};

pub use find::PromptMatch;
pub use resume::{SessionPick, SessionResume, session_id};

/// The OTLP signal of `/ingest/{source}/v1/{signal}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Logs,
    Metrics,
}

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
    /// Route latencies, process samples and warnings (see [`crate::health`]).
    health: Arc<Health>,
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
}

impl Service {
    /// Open the store at `settings.db_path()` and create the receiver token if there is none.
    pub fn new(settings: Settings, options: AppOptions, bus: Bus) -> ApiResult<Self> {
        settings.token()?;
        let store = Store::open(&settings.db_path())?.with_clock(crate::clock::for_store(&options.clock));
        let inner = Inner {
            specs: SpecCache::new(settings.clone()),
            preferences: PreferenceFile::new(&settings.home, settings.env.clone()),
            indexer: Mutex::new(None),
            closing: CancellationToken::new(),
            started_at: options.clock.now(),
            health: Arc::default(),
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

    /// What the access log records and the status reports (see [`crate::health`]).
    pub fn health(&self) -> Arc<Health> {
        Arc::clone(&self.inner.health)
    }

    /// One health sample: the process now, the counters so far; logs and keeps what `monitor`
    /// finds worth a warning. The daemon calls it every [`crate::health::SAMPLE_EVERY`].
    pub fn sample_health(&self, monitor: &mut Monitor, sampler: &mut ProcessSampler) {
        let process = sampler.sample();
        let health = &self.inner.health;
        health.set_process(process);
        let (errors, slow) = health.totals();
        let sample = Sample { process, writer: self.inner.store.writer_stats(), errors, slow };
        let at = iso(self.inner.now());
        for (kind, message) in monitor.observe(sample, std::time::Instant::now()) {
            health.warn(at.clone(), kind, message);
        }
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
                health: inner.health.snapshot(inner.store.writer_stats()),
            })
        })
        .await
    }
}
