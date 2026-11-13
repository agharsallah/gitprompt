//! Capturing payloads: hook and OTLP requests, and the spool the forwarder filled while the daemon
//! was down. Every capture marks the turns that got events and announces what was stored.

use std::path::Path;

use gitprompt_capture::pipeline::{Arrival, CaptureOptions, Captured, GitProbe, Observer, capture_then, mark_turns};
use gitprompt_capture::probe::VersionProbe;
use gitprompt_capture::spool::{Spool, drain_into_pipeline};
use gitprompt_core::spec::{Format, Spec};
use gitprompt_core::{Error, Json, JsonMap};

use super::{Inner, Service, Signal};
use crate::bus::Notice;
use crate::error::ApiResult;
use crate::options::{GitProvider, ToolVersionProbe};

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
    /// Run `f` with the pipeline's observer of the app's git and tool-version probes.
    fn observed<T>(&self, f: impl FnOnce(&Observer<'_>) -> T) -> T {
        let (git, versions) = (GitAdapter(self.options.git.as_ref()), VersionAdapter(self.options.tool_version.as_ref()));
        f(&Observer { git: &git, versions: &versions })
    }

    /// Capture one request body for `spec` (marking the turns that got events for the index in the
    /// same transaction) and announce what was stored. The git and `--version` probes run before
    /// that transaction, so they never hold the writer (see `capture_then`).
    fn capture(&self, spec: &Spec, payload: &Json) -> ApiResult<Notice> {
        let options = CaptureOptions::for_source(&self.preferences.effective(), &spec.id);
        if !options.enabled {
            return Ok(Notice::default());
        }
        let captured = self.observed(|observer| capture_then(spec, &self.store, Arrival::now(payload), options, observer, mark_turns))?;
        Ok(self.announce(&captured))
    }

    fn announce(&self, captured: &Captured) -> Notice {
        let notice = Notice::from(captured.notice());
        if !captured.is_empty() {
            self.bus.publish(notice);
        }
        notice
    }
}

impl Service {
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

    /// Ingest every payload the hook forwarder spooled while the daemon was down, oldest first,
    /// removing each one delivered and marking its turns for the index as it is stored, until
    /// `stop` says so (checked between payloads; the rest stays queued). Announces what was stored
    /// once at the end. Returns how many were delivered.
    pub async fn drain_spool(&self, stop: impl Fn() -> bool + Send + 'static) -> ApiResult<u64> {
        self.run(move |inner| {
            let set = inner.spec_set();
            let spool = Spool::of(&inner.settings);
            let preferences = inner.preferences.effective();
            let (report, captured) =
                inner.observed(|observer| drain_into_pipeline(&spool, &set.specs, &inner.store, &preferences, observer, &stop))?;
            inner.announce(&captured);
            Ok(report.values().map(|d| d.ingested as u64).sum())
        })
        .await
    }
}
