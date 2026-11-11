//! What the server reads from the outside world, injectable so tests (and the desktop shell) control
//! it: the clock, the tool-version probe, git. Production defaults are [`AppOptions::default`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use jiff::Timestamp;

use gitprompt_capture::probe::{VersionCache, VersionProbe};
use gitprompt_core::JsonMap;
use gitprompt_core::git::{self, Commit, CommitsAfter, GitCache};
use gitprompt_core::preferences::APPLIES_AT_START;
use gitprompt_core::spec::Spec;

use crate::dashboard::Dashboard;

/// "Now", as the server sees it: the heartbeat window of `/api/sources`, the `before` a
/// delete-events dry run fixes, compact and retention cutoffs, export `generated_at` and file
/// names. (Row `received_at` stamps are the store's own.)
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// Always the same instant (tests).
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

/// The installed version of a tool (`version_cmd` of its spec), recorded in a prompt's context at
/// ingest and shown on the Connectors page. `None`: not installed, or no version command.
pub trait ToolVersionProbe: Send + Sync {
    fn version(&self, spec: &Spec) -> Option<String>;
}

/// Runs the spec's `version_cmd` (`gitprompt_capture::probe::VersionCache`: remembered for
/// `VERSION_MAX_AGE`, killed after `VERSION_TIMEOUT`, so a slow or missing tool never holds ingest
/// for long). Clones share one cache.
#[derive(Clone)]
pub struct ProbeVersions {
    cache: Arc<VersionCache>,
}

impl ProbeVersions {
    /// A fresh cache running the real commands.
    pub fn new() -> Self {
        ProbeVersions { cache: Arc::new(VersionCache::system()) }
    }
}

impl Default for ProbeVersions {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProbeVersions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeVersions").finish_non_exhaustive()
    }
}

impl ToolVersionProbe for ProbeVersions {
    fn version(&self, spec: &Spec) -> Option<String> {
        self.cache.tool_version(spec)
    }
}

/// The same version for every tool (tests). `FixedVersion(None)`: no tool installed.
#[derive(Clone, Debug)]
pub struct FixedVersion(pub Option<String>);

impl ToolVersionProbe for FixedVersion {
    fn version(&self, _spec: &Spec) -> Option<String> {
        self.0.clone()
    }
}

/// Git facts: the context recorded at ingest, the commits after a prompt (prompt detail), the
/// history of a story. Signatures mirror `gitprompt_core::git`.
pub trait GitProvider: Send + Sync {
    fn context(&self, cwd: Option<&Path>) -> JsonMap;
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter;
    fn history(&self, root: &Path, since: &str, until: &str) -> Vec<Commit>;
}

/// The user's `git` CLI, with the context memoized for `ttl` per directory (`core::git::GitCache`).
#[derive(Debug)]
pub struct SystemGit {
    cache: GitCache,
}

impl SystemGit {
    pub fn new(ttl: Duration) -> Self {
        SystemGit { cache: GitCache::new(ttl) }
    }
}

impl GitProvider for SystemGit {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        self.cache.context(cwd)
    }
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
        git::commits(root, start, end, since)
    }
    fn history(&self, root: &Path, since: &str, until: &str) -> Vec<Commit> {
        git::history(root, since, until)
    }
}

/// No repository anywhere: empty context, no commits.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoGit;

impl GitProvider for NoGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        JsonMap::new()
    }
    fn commits(&self, _root: &Path, _start: Option<&str>, end: &str, _since: &str) -> CommitsAfter {
        CommitsAfter { until: end.to_owned(), ..CommitsAfter::default() }
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        Vec::new()
    }
}

#[derive(Clone)]
pub struct AppOptions {
    pub clock: Arc<dyn Clock>,
    pub tool_version: Arc<dyn ToolVersionProbe>,
    pub git: Arc<dyn GitProvider>,
    /// The built dashboard (`pnpm --dir web build`), served at `/` with its SPA fallback: a
    /// folder, or the copy compiled into the binary ([`Dashboard::Embedded`]).
    /// `None`: API only (unknown paths are 404).
    pub dashboard: Option<Dashboard>,
    /// Notices a stalled live-update subscriber may fall behind before older ones are dropped.
    pub bus_backlog: usize,
    /// Preference keys whose change needs a restart (`SettingsSaved::restart_required`).
    /// Default: `gitprompt_core::preferences::APPLIES_AT_START`.
    pub applies_at_start: Vec<String>,
    /// The `gitprompt` executable hooks should run (the desktop app's bundled sidecar). Set:
    /// connect and token rotation install the native forwarder form of each hook command
    /// (`gitprompt_capture::Connection::with_forwarder`), which spools payloads while GitPrompt is
    /// down, and the Connectors page compares installed entries against that form. `None`: the
    /// spec's curl commands.
    pub forwarder: Option<PathBuf>,
}

impl std::fmt::Debug for AppOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppOptions")
            .field("dashboard", &self.dashboard.is_some())
            .field("bus_backlog", &self.bus_backlog)
            .finish_non_exhaustive()
    }
}

impl Default for AppOptions {
    /// System clock, [`ProbeVersions`], [`SystemGit`] (2 s cache), no dashboard, backlog 100,
    /// `APPLIES_AT_START`, curl hooks (no forwarder).
    fn default() -> Self {
        AppOptions {
            clock: Arc::new(SystemClock),
            tool_version: Arc::new(ProbeVersions::new()),
            git: Arc::new(SystemGit::new(Duration::from_secs(2))),
            dashboard: None,
            bus_backlog: 100,
            applies_at_start: APPLIES_AT_START.iter().map(|k| (*k).to_owned()).collect(),
            forwarder: None,
        }
    }
}
