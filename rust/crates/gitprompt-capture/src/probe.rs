//! What GitPrompt reads about a tool on this machine: is it installed, which version, and does
//! policy (organization or user settings) allow GitPrompt to connect. Read-only. Port of
//! `sources/probe.py`; the command runner and the clock are injected (new) so nothing here needs
//! the real tool or the real time in tests.

use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::Settings;
use gitprompt_core::spec::{Capability, Spec};

/// How long a tool's `--version` is remembered.
pub const VERSION_MAX_AGE: Duration = Duration::from_secs(300);

/// How long a `--version` command may run before it counts as "no version".
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether any of the spec's `detect` paths (expanded) exists.
pub fn detected(_spec: &Spec, _settings: &Settings) -> bool {
    todo!()
}

/// The version in a `--version` output: the first dotted number (`"2.1.317 (Claude Code)"` ->
/// `2.1.317`, `"GitHub Copilot CLI 1.0.102."` -> `1.0.102`, a `-rc.1`/`+build` suffix kept), else the
/// output's first word; `None` for a blank output.
pub fn version_of(_output: &str) -> Option<String> {
    todo!()
}

/// Capabilities the installed version doesn't have yet (spec `capability_since`), with the version
/// that adds each. An unknown or unparsable version is given the benefit of the doubt (`{}`).
/// Versions compare by their numeric release parts (`0.164.0` > `0.99`), ignoring `-pre`/`+build`.
pub fn missing_capabilities(_spec: &Spec, _version: Option<&str>) -> IndexMap<Capability, String> {
    todo!()
}

// --- tool version ----------------------------------------------------------------------------

/// Runs a command and returns its stdout; `None` when it can't be started, fails to finish within
/// `timeout`, or isn't valid UTF-8. The out-of-process edge of [`VersionCache`].
pub trait CommandRunner: Send + Sync {
    fn run(&self, cmd: &[String], timeout: Duration) -> Option<String>;
}

/// Runs the command as a child process (production).
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, _cmd: &[String], _timeout: Duration) -> Option<String> {
        todo!()
    }
}

/// A monotonic clock, injectable so cache expiry is tested without waiting.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// `Instant::now()` (production).
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        todo!()
    }
}

/// The installed version of a tool, as the pipeline stamps it on every prompt.
pub trait VersionProbe: Send + Sync {
    fn tool_version(&self, spec: &Spec) -> Option<String>;
}

/// A tool's `--version` (spec `version_cmd`, parsed by [`version_of`]), remembered per command for
/// `max_age`, so a burst of prompts runs the tool once. A spec without `version_cmd` has no version
/// and runs nothing. A failed run is remembered too (`None`). Thread-safe.
pub struct VersionCache {
    pub runner: Arc<dyn CommandRunner>,
    pub clock: Arc<dyn Clock>,
    pub max_age: Duration,
}

impl VersionCache {
    pub fn new(runner: Arc<dyn CommandRunner>, clock: Arc<dyn Clock>, max_age: Duration) -> Self {
        VersionCache { runner, clock, max_age }
    }

    /// [`SystemRunner`], [`SystemClock`], [`VERSION_MAX_AGE`].
    pub fn system() -> Self {
        todo!()
    }
}

impl VersionProbe for VersionCache {
    fn tool_version(&self, _spec: &Spec) -> Option<String> {
        todo!()
    }
}

// --- policy ----------------------------------------------------------------------------------

/// What settings files and the environment say about connecting a source.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReport {
    /// GitPrompt must not connect. `"<path> is set in <file>"` (`block_if`: the value at `path` is
    /// one of the listed values, or a list containing one), `"<path> is already set in <file>"`
    /// (`block_if_set`: a value that is not GitPrompt's own receiver URL for this source).
    pub blocks: Vec<String>,
    /// It may connect, but something may interfere: `"<path> is set in <file>: it must allow
    /// <receiver>"` (`warn_if_set`), `"<VAR> is set in this shell's environment: ..."` (`warn_env`,
    /// read from `settings.env`).
    pub warnings: Vec<String>,
}

/// Blocks and warnings found in the spec's `policy.files` (each pattern expanded, then globbed with
/// `*`, matches in sorted order) and in the environment. A settings file is parsed as TOML when its
/// name ends in `.toml`, JSON otherwise; a file that can't be read or parsed, or isn't an object, is
/// skipped (the tool can't apply it either). GitPrompt reports policy; it never works around it.
pub fn check_policy(_spec: &Spec, _settings: &Settings) -> PolicyReport {
    todo!()
}
