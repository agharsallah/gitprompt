//! What GitPrompt reads about a tool on this machine: is it installed, which version, and does
//! policy (organization or user settings) allow GitPrompt to connect. Read-only. Port of
//! `sources/probe.py`; the command runner and the clock are injected (new) so nothing here needs
//! the real tool or the real time in tests.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::Json;
use gitprompt_core::Settings;
use gitprompt_core::json::pluck;
use gitprompt_core::spec::{Capability, Pattern, Policy, Spec, expand};

/// How long a tool's `--version` is remembered.
pub const VERSION_MAX_AGE: Duration = Duration::from_secs(300);

/// How long a `--version` command may run before it counts as "no version".
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether any of the spec's `detect` paths (expanded) exists.
pub fn detected(spec: &Spec, settings: &Settings) -> bool {
    spec.detect.iter().any(|path| expand(path, settings).exists())
}

/// The version in a `--version` output: the first dotted number (`"2.1.317 (Claude Code)"` ->
/// `2.1.317`, `"GitHub Copilot CLI 1.0.102."` -> `1.0.102`, a `-rc.1`/`+build` suffix kept), else the
/// output's first word; `None` for a blank output.
pub fn version_of(output: &str) -> Option<String> {
    dotted_number(output).or_else(|| output.split_whitespace().next().map(str::to_owned))
}

/// The leftmost `\d+(\.\d+)+([-+][\w.]+)?` of `text`, trailing dots removed.
fn dotted_number(text: &str) -> Option<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let is_digit = |i: usize| chars.get(i).is_some_and(|(_, c)| c.is_ascii_digit());
    let is_word = |i: usize| chars.get(i).is_some_and(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '.');
    let at = |i: usize| chars.get(i).map(|(_, c)| *c);
    let mut start = 0;
    while start < chars.len() {
        if !is_digit(start) {
            start += 1;
            continue;
        }
        let mut end = start;
        while is_digit(end) {
            end += 1;
        }
        // A run of digits starts a version only when a `.<digit>` follows it.
        if !(at(end) == Some('.') && is_digit(end + 1)) {
            start = end;
            continue;
        }
        while at(end) == Some('.') && is_digit(end + 1) {
            end += 1;
            while is_digit(end) {
                end += 1;
            }
        }
        if matches!(at(end), Some('-' | '+')) && is_word(end + 1) {
            end += 1;
            while is_word(end) {
                end += 1;
            }
        }
        let byte_end = chars.get(end).map_or(text.len(), |(i, _)| *i);
        let byte_start = chars.get(start)?.0;
        return Some(text.get(byte_start..byte_end)?.trim_end_matches('.').to_owned());
    }
    None
}

/// The numeric release parts of a version (`0.164.0-alpha.2` -> `[0, 164, 0]`), ignoring `-pre`
/// and `+build`.
fn release(version: &str) -> Vec<u64> {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    core.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        // Digits only, so a parse can fail only by overflowing: such a part is "very large".
        .map(|part| part.parse().unwrap_or(u64::MAX))
        .collect()
}

/// Capabilities the installed version doesn't have yet (spec `capability_since`), with the version
/// that adds each. An unknown or unparsable version is given the benefit of the doubt (`{}`).
/// Versions compare by their numeric release parts (`0.164.0` > `0.99`), ignoring `-pre`/`+build`.
pub fn missing_capabilities(spec: &Spec, version: Option<&str>) -> IndexMap<Capability, String> {
    let installed = version.map(release).unwrap_or_default();
    if installed.is_empty() {
        return IndexMap::new();
    }
    spec.capability_since.iter().filter(|(_, since)| installed < release(since)).map(|(c, since)| (*c, since.clone())).collect()
}

// --- tool version ----------------------------------------------------------------------------

/// Runs a command and returns its stdout; `None` when it can't be started, fails to finish within
/// `timeout`, or isn't valid UTF-8. The out-of-process edge of [`VersionCache`].
pub trait CommandRunner: Send + Sync {
    fn run(&self, cmd: &[String], timeout: Duration) -> Option<String>;
}

/// Runs the command as a child process (production). Its stdin and stderr are closed; it is killed
/// when it outlives the timeout.
#[derive(Debug)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, cmd: &[String], timeout: Duration) -> Option<String> {
        let (program, args) = cmd.split_first()?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| tracing::debug!(%program, error = %e, "version command could not start"))
            .ok()?;
        let stdout = child.stdout.take()?;
        let output = read_with_timeout(stdout, timeout);
        if output.is_none() {
            tracing::debug!(%program, "version command timed out");
            let _ = child.kill(); // it may have exited meanwhile; nothing to do either way
        }
        let _ = child.wait(); // reap it; the exit status doesn't matter, only what it printed
        String::from_utf8(output?).ok()
    }
}

/// Everything a reader yields until EOF, or `None` when that takes longer than `timeout`. Read on
/// its own thread, so a full pipe never blocks the command.
fn read_with_timeout(mut reader: impl Read + Send + 'static, timeout: Duration) -> Option<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let read = reader.read_to_end(&mut buf).map(|_| buf);
        let _ = tx.send(read); // the receiver is gone after a timeout
    });
    rx.recv_timeout(timeout).ok()?.ok()
}

/// A monotonic clock, injectable so cache expiry is tested without waiting.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// `Instant::now()` (production).
#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// The installed version of a tool, as the pipeline stamps it on every prompt.
pub trait VersionProbe: Send + Sync {
    fn tool_version(&self, spec: &Spec) -> Option<String>;
}

/// One remembered `--version`: when it ran, what it gave, and whether a newer run is under way.
#[derive(Debug)]
struct Remembered {
    at: Instant,
    version: Option<String>,
    refreshing: bool,
}

/// The slot of one command: empty until its first run finishes.
type Slot = Arc<Mutex<Option<Remembered>>>;

/// A tool's `--version` (spec `version_cmd`, parsed by [`version_of`]), remembered per command, so a
/// burst of prompts runs the tool once. A spec without `version_cmd` has no version and runs
/// nothing. A failed run is remembered too (`None`).
///
/// Only the very first ask of a command waits for it (callers asking for the same command meanwhile
/// wait for that answer; other commands are not held up). Once an answer is older than `max_age`,
/// it is still returned at once while the command runs again on its own thread
/// (stale-while-revalidate): a `--version` can take seconds (a Node.js CLI starting up), and the
/// callers are request handlers and ingest, which must not wait for it.
pub struct VersionCache {
    pub runner: Arc<dyn CommandRunner>,
    pub clock: Arc<dyn Clock>,
    pub max_age: Duration,
    entries: Mutex<HashMap<Vec<String>, Slot>>,
}

impl std::fmt::Debug for VersionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VersionCache").field("max_age", &self.max_age).finish_non_exhaustive()
    }
}

impl VersionCache {
    pub fn new(runner: Arc<dyn CommandRunner>, clock: Arc<dyn Clock>, max_age: Duration) -> Self {
        VersionCache { runner, clock, max_age, entries: Mutex::new(HashMap::new()) }
    }

    /// [`SystemRunner`], [`SystemClock`], [`VERSION_MAX_AGE`].
    pub fn system() -> Self {
        Self::new(Arc::new(SystemRunner), Arc::new(SystemClock), VERSION_MAX_AGE)
    }

    /// The slot of one command (created empty on first use).
    fn entry(&self, cmd: &[String]) -> Slot {
        // A poisoned lock only means another thread panicked mid-lookup; the map is still valid.
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.entry(cmd.to_vec()).or_default().clone()
    }

    fn run(runner: &dyn CommandRunner, cmd: &[String]) -> Option<String> {
        runner.run(cmd, VERSION_TIMEOUT).and_then(|out| version_of(&out))
    }

    /// Run `cmd` again on its own thread and store the answer in `slot`. False when no thread could
    /// be started (the stale answer stays; the next ask tries again).
    fn refresh_in_background(&self, cmd: &[String], slot: Slot) -> bool {
        let (runner, clock, cmd) = (Arc::clone(&self.runner), Arc::clone(&self.clock), cmd.to_vec());
        std::thread::Builder::new()
            .name("gitprompt-version".into())
            .spawn(move || {
                // Dropped on every exit, a panicking runner included: the slot must never stay
                // marked as refreshing, or that command would never run again.
                let guard = Refreshing(slot);
                let version = Self::run(runner.as_ref(), &cmd);
                guard.finish(Remembered { at: clock.now(), version, refreshing: false });
            })
            .map_err(|e| tracing::debug!(error = %e, "could not start a version refresh"))
            .is_ok()
    }
}

/// A refresh under way: stores its answer, or (dropped without one) lets the next ask retry while
/// keeping the old answer.
struct Refreshing(Slot);

impl Refreshing {
    fn finish(self, answer: Remembered) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(answer);
    }
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        if let Some(remembered) = self.0.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            remembered.refreshing = false;
        }
    }
}

impl VersionProbe for VersionCache {
    fn tool_version(&self, spec: &Spec) -> Option<String> {
        if spec.version_cmd.is_empty() {
            return None;
        }
        let entry = self.entry(&spec.version_cmd);
        let mut slot = entry.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(remembered) = slot.as_mut() {
            let stale = self.clock.now().saturating_duration_since(remembered.at) >= self.max_age;
            if stale && !remembered.refreshing {
                remembered.refreshing = self.refresh_in_background(&spec.version_cmd, Arc::clone(&entry));
            }
            return remembered.version.clone();
        }
        let version = Self::run(self.runner.as_ref(), &spec.version_cmd);
        *slot = Some(Remembered { at: self.clock.now(), version: version.clone(), refreshing: false });
        version
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

/// One settings file named by `policy.files`, as the rules below see it.
struct SettingsFile<'a> {
    name: String,
    doc: Json,
    policy: &'a Policy,
    /// Matches GitPrompt's own receiver URL for this source.
    ours: &'a Pattern,
    receiver: &'a str,
}

/// A rule reads one settings file and adds to the report. To support a new policy key: add a field
/// to `Policy` (core spec) and a rule here.
type Rule = fn(&SettingsFile<'_>, &mut PolicyReport);

const FILE_RULES: [Rule; 3] = [block_if, block_if_set, warn_if_set];

/// Blocks and warnings found in the spec's `policy.files` (each pattern expanded, then globbed with
/// `*`, matches in sorted order) and in the environment. A settings file is parsed as TOML when its
/// name ends in `.toml`, JSON otherwise; a file that can't be read or parsed, or isn't an object, is
/// skipped (the tool can't apply it either). GitPrompt reports policy; it never works around it.
pub fn check_policy(spec: &Spec, settings: &Settings) -> PolicyReport {
    let mut report = PolicyReport::default();
    let ours = spec.marker();
    let receiver = settings.receiver();
    for path in spec.policy.files.iter().flat_map(|pattern| matching_files(&expand(pattern, settings))) {
        let Some(doc) = parse_settings(&path) else { continue };
        let file = SettingsFile { name: path.display().to_string(), doc, policy: &spec.policy, ours: &ours, receiver: &receiver };
        for rule in FILE_RULES {
            rule(&file, &mut report);
        }
    }
    report.warnings.extend(
        spec.policy
            .warn_env
            .iter()
            .filter(|var| settings.env.get(var).is_some())
            .map(|var| format!("{var} is set in this shell's environment: where it is also set, it wins over the settings file")),
    );
    report
}

/// Existing files matching a (glob) pattern, sorted. An invalid pattern matches nothing.
fn matching_files(pattern: &Path) -> Vec<PathBuf> {
    let Some(pattern) = pattern.to_str() else { return Vec::new() };
    let mut found: Vec<PathBuf> = match glob::glob(pattern) {
        Ok(paths) => paths.filter_map(Result::ok).filter(|p| p.is_file()).collect(),
        Err(e) => {
            tracing::warn!(%pattern, error = %e, "invalid policy file pattern");
            Vec::new()
        }
    };
    found.sort();
    found
}

/// A settings file as a JSON object (TOML by its `.toml` suffix); `None` when it can't be read,
/// parsed, or isn't an object.
fn parse_settings(path: &Path) -> Option<Json> {
    let text = std::fs::read_to_string(path).ok()?;
    let is_toml = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
    let doc = if is_toml {
        let table: toml::Table = toml::from_str(&text).ok()?;
        serde_json::to_value(table).ok()?
    } else {
        serde_json::from_str(&text).ok()?
    };
    doc.is_object().then_some(doc)
}

/// The value at a dotted path; JSON `null` counts as unset.
fn setting<'a>(doc: &'a Json, path: &str) -> Option<&'a Json> {
    pluck(doc, path).filter(|value| !value.is_null())
}

fn block_if(file: &SettingsFile<'_>, report: &mut PolicyReport) {
    for (path, values) in &file.policy.block_if {
        if setting(&file.doc, path).is_some_and(|value| matches_any(value, values)) {
            report.blocks.push(format!("{path} is set in {}", file.name));
        }
    }
}

fn block_if_set(file: &SettingsFile<'_>, report: &mut PolicyReport) {
    for path in &file.policy.block_if_set {
        if setting(&file.doc, path).is_some_and(|value| is_truthy(value) && !file.ours.is_match(&text_of(value))) {
            report.blocks.push(format!("{path} is already set in {}", file.name));
        }
    }
}

fn warn_if_set(file: &SettingsFile<'_>, report: &mut PolicyReport) {
    for path in &file.policy.warn_if_set {
        if setting(&file.doc, path).is_some() {
            report.warnings.push(format!("{path} is set in {}: it must allow {}", file.name, file.receiver));
        }
    }
}

/// The value is one of `values`, or a list holding one of them.
fn matches_any(value: &Json, values: &[Json]) -> bool {
    values.contains(value) || value.as_array().is_some_and(|items| values.iter().any(|v| items.contains(v)))
}

/// Python truthiness: not `false`, `0`, `""`, `[]` or `{}`.
fn is_truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Json::String(s) => !s.is_empty(),
        Json::Array(items) => !items.is_empty(),
        Json::Object(map) => !map.is_empty(),
    }
}

/// A string as is, anything else as JSON text.
fn text_of(value: &Json) -> String {
    value.as_str().map_or_else(|| value.to_string(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_suffix_needs_a_word_character_after_its_sign() {
        assert_eq!(version_of("tool 1.2.3- beta").as_deref(), Some("1.2.3"));
        assert_eq!(version_of("v1.2+build.7").as_deref(), Some("1.2+build.7"));
    }

    #[test]
    fn a_number_without_a_dot_is_not_a_version() {
        assert_eq!(version_of("build 42 of 1.5").as_deref(), Some("1.5"));
        assert_eq!(version_of("42").as_deref(), Some("42"));
    }

    #[test]
    fn release_parts_ignore_pre_release_and_build_suffixes() {
        assert_eq!(release("0.167.0-alpha.2"), vec![0, 167, 0]);
        assert_eq!(release("1.2+build.7"), vec![1, 2]);
        assert!(release("0.99") < release("0.164.0"));
    }
}
