//! What the server reads from the outside world, injectable so tests (and the desktop shell) control
//! it: the clock, the tool-version probe, git. Production defaults are [`AppOptions::default`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use jiff::Timestamp;

use gitprompt_core::JsonMap;
use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_core::spec::Spec;

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
        todo!()
    }
}

/// Always the same instant (tests).
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        todo!()
    }
}

/// The installed version of a tool (`version_cmd` of its spec), recorded in a prompt's context at
/// ingest and shown on the Connectors page. `None`: not installed, or no version command.
pub trait ToolVersionProbe: Send + Sync {
    fn version(&self, spec: &Spec) -> Option<String>;
}

/// Runs the spec's `version_cmd` (cached for a while; a slow or missing tool never blocks ingest).
#[derive(Clone, Copy, Debug, Default)]
pub struct ProbeVersions;

impl ToolVersionProbe for ProbeVersions {
    fn version(&self, _spec: &Spec) -> Option<String> {
        todo!()
    }
}

/// The same version for every tool (tests). `FixedVersion(None)`: no tool installed.
#[derive(Clone, Debug)]
pub struct FixedVersion(pub Option<String>);

impl ToolVersionProbe for FixedVersion {
    fn version(&self, _spec: &Spec) -> Option<String> {
        todo!()
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
pub struct SystemGit {
    _ttl: Duration,
}

impl SystemGit {
    pub fn new(ttl: Duration) -> Self {
        SystemGit { _ttl: ttl }
    }
}

impl GitProvider for SystemGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        todo!()
    }
    fn commits(&self, _root: &Path, _start: Option<&str>, _end: &str, _since: &str) -> CommitsAfter {
        todo!()
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        todo!()
    }
}

/// No repository anywhere: empty context, no commits.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoGit;

impl GitProvider for NoGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        todo!()
    }
    fn commits(&self, _root: &Path, _start: Option<&str>, _end: &str, _since: &str) -> CommitsAfter {
        todo!()
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        todo!()
    }
}

#[derive(Clone)]
pub struct AppOptions {
    pub clock: Arc<dyn Clock>,
    pub tool_version: Arc<dyn ToolVersionProbe>,
    pub git: Arc<dyn GitProvider>,
    /// The built dashboard (`pnpm --dir web build`), served at `/` with its SPA fallback.
    /// `None`: API only (unknown paths are 404).
    pub dashboard: Option<PathBuf>,
    /// Notices a stalled live-update subscriber may fall behind before older ones are dropped.
    pub bus_backlog: usize,
    /// Preference keys whose change needs a restart (`SettingsSaved::restart_required`).
    /// Default: `gitprompt_core::preferences::APPLIES_AT_START`.
    pub applies_at_start: Vec<String>,
}

impl Default for AppOptions {
    /// System clock, [`ProbeVersions`], [`SystemGit`] (2 s cache), no dashboard, backlog 100.
    fn default() -> Self {
        todo!()
    }
}
