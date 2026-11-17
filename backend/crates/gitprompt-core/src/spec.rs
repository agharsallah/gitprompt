//! Source specs: everything GitPrompt knows about a tool lives in a YAML file, not in code.
//!
//! Bundled specs are compiled into the binary ([`BUNDLED`]); a file with the same name in
//! `<home>/sources/` overrides one. Each file loads on its own: a broken file is reported with its
//! error and skipped, and a broken override of a bundled spec falls back to the bundled file
//! (ADR-0017). Port of `sources/spec.py`, plus [`SpecCache`] (reloads only when a user spec file
//! changes, instead of re-reading YAML on every request).

mod expand;
mod model;
mod parse;
mod pattern;
mod resume;
mod validate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_norway::Mapping as YamlMap;
use ts_rs::TS;

pub use expand::{RESUME_PLACEHOLDERS, expand, shell_quote};
pub use model::{
    Capability, EventName, Format, Install, Join, Origin, OriginRule, Policy, ResumeSpec, Spec, SurfaceInfo, SurfaceRef, SurfaceRule,
    ToolKind, Usage, UsageValue, When, by_name, matches,
};
use parse::bundled_text;
pub use parse::{bundled_defaults, parse_spec};
pub use pattern::Pattern;
pub use resume::Resume;

use crate::{Error, Result, Settings};

/// Shared settings every spec inherits (file name among [`BUNDLED`]).
pub const DEFAULTS: &str = "_defaults.yaml";

/// (file name, contents) of every bundled spec, `_defaults.yaml` included.
pub const BUNDLED: &[(&str, &str)] = &[
    ("_defaults.yaml", include_str!("../specs/_defaults.yaml")),
    ("claude-code-otel.yaml", include_str!("../specs/claude-code-otel.yaml")),
    ("claude-code.yaml", include_str!("../specs/claude-code.yaml")),
    ("codex.yaml", include_str!("../specs/codex.yaml")),
    ("copilot-cli.yaml", include_str!("../specs/copilot-cli.yaml")),
    ("cursor.yaml", include_str!("../specs/cursor.yaml")),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SpecOrigin {
    Bundled,
    User,
}

/// Where a spec was read from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFile {
    /// e.g. `claude-code.yaml`
    pub name: String,
    /// The user file's path, or `bundled:<name>` for a bundled spec.
    pub path: String,
    pub origin: SpecOrigin,
    /// A user file with the name of a bundled one.
    pub overrides: bool,
}

/// A spec file that can't be used, and what GitPrompt does instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecProblem {
    pub file: SpecFile,
    pub error: String,
    /// Id of the bundled spec used instead, if any.
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SpecSet {
    pub specs: BTreeMap<String, Arc<Spec>>,
    /// spec id -> the file it was read from.
    pub files: BTreeMap<String, SpecFile>,
    pub problems: Vec<SpecProblem>,
}

impl SpecSet {
    pub fn get(&self, id: &str) -> Option<&Arc<Spec>> {
        self.specs.get(id)
    }
}

/// The text of a spec file (bundled contents, or the user file read from disk).
pub fn read_spec_file(file: &SpecFile) -> Result<String> {
    match file.origin {
        SpecOrigin::Bundled => {
            bundled_text(&file.name).map(str::to_owned).ok_or_else(|| Error::NotFound(format!("no bundled source spec {}", file.name)))
        }
        SpecOrigin::User => Ok(std::fs::read_to_string(&file.path)?),
    }
}

/// Every usable spec, the file each came from, and the files that can't be used. Never fails over a
/// spec file: an unreadable/invalid one becomes a problem (a user override falls back to the bundled
/// spec of the same name), a duplicate id is a problem, a `part_of` naming a missing or non-main spec
/// is a problem. `_defaults.yaml` in the user directory is ignored (not overridable).
///
/// Files are tried by name, in name order; for each name the user file first, then the bundled one.
pub fn load_spec_set(settings: &Settings) -> SpecSet {
    let defaults = bundled_defaults();
    let mut out = SpecSet::default();
    for candidates in candidates(settings).into_values() {
        load_one(&candidates, &defaults, &mut out);
    }
    drop_orphan_layers(&mut out);
    out.problems.iter().for_each(warn_unusable);
    out
}

/// Logs a spec file that can't be used, and the bundled spec used instead, if any.
fn warn_unusable(problem: &SpecProblem) {
    let instead = problem.fallback.as_ref().map(|fallback| format!(" (using the bundled {fallback})")).unwrap_or_default();
    tracing::warn!("{}{instead}", problem.error);
}

/// The usable specs by id.
pub fn load_specs(settings: &Settings) -> BTreeMap<String, Arc<Spec>> {
    load_spec_set(settings).specs
}

/// Per file name, the files to try in order: the user's override first, then the bundled file.
fn candidates(settings: &Settings) -> BTreeMap<String, Vec<SpecFile>> {
    let bundled: Vec<&str> = BUNDLED.iter().map(|(name, _)| *name).filter(|name| *name != DEFAULTS).collect();
    let mut out: BTreeMap<String, Vec<SpecFile>> = BTreeMap::new();
    for (name, path) in user_spec_paths(&settings.user_sources()) {
        let overrides = bundled.contains(&name.as_str());
        let file = SpecFile { path: path.display().to_string(), origin: SpecOrigin::User, overrides, name: name.clone() };
        out.entry(name).or_default().push(file);
    }
    for name in bundled {
        let file = SpecFile { name: name.into(), path: format!("bundled:{name}"), origin: SpecOrigin::Bundled, overrides: false };
        out.entry(name.into()).or_default().push(file);
    }
    out
}

/// (file name, path) of the user's `*.yaml` files (not `_defaults.yaml`, which is not overridable).
fn user_spec_paths(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .filter(|(name, path)| is_user_spec_name(name) && path.is_file())
        .collect()
}

#[expect(clippy::case_sensitive_file_extension_comparisons, reason = "spec files are lowercase `.yaml`, as the Python version matched")]
fn is_user_spec_name(name: &str) -> bool {
    name.ends_with(".yaml") && name != DEFAULTS
}

/// The first candidate that parses wins; a failure before it is a problem (with its fallback).
fn load_one(candidates: &[SpecFile], defaults: &YamlMap, out: &mut SpecSet) {
    let mut failed: Option<SpecProblem> = None;
    for file in candidates {
        let spec = match read_and_parse(file, defaults) {
            Ok(spec) => spec,
            Err(error) => {
                failed.get_or_insert_with(|| SpecProblem { file: file.clone(), error, fallback: None });
                continue;
            }
        };
        if let Some(problem) = &mut failed {
            problem.fallback = Some(spec.id.clone());
        }
        if let Some(first) = out.files.get(&spec.id) {
            let error = format!("source spec {}: id '{}' is already used by {}", file.name, spec.id, first.name);
            out.problems.push(SpecProblem { file: file.clone(), error, fallback: None });
        } else {
            out.files.insert(spec.id.clone(), file.clone());
            out.specs.insert(spec.id.clone(), Arc::new(spec));
        }
        break;
    }
    out.problems.extend(failed);
}

/// The spec in a file, or why it can't be used (naming the file).
fn read_and_parse(file: &SpecFile, defaults: &YamlMap) -> std::result::Result<Spec, String> {
    let text = read_spec_file(file).map_err(|e| format!("source spec {} can't be read: {e}", file.name))?;
    parse_spec(&file.name, &text, defaults).map_err(|e| e.to_string())
}

/// Layers whose `part_of` doesn't name a loaded main spec become problems.
fn drop_orphan_layers(out: &mut SpecSet) {
    let layers: Vec<(String, String)> = out.specs.values().filter_map(|spec| Some((spec.id.clone(), spec.part_of.clone()?))).collect();
    for (id, part_of) in layers {
        let main_exists = out.specs.get(&part_of).is_some_and(|main| main.part_of.is_none());
        if main_exists {
            continue;
        }
        out.specs.remove(&id);
        if let Some(file) = out.files.remove(&id) {
            let error = format!("source spec {}: part_of must name a main source spec, not '{part_of}'", file.name);
            out.problems.push(SpecProblem { file, error, fallback: None });
        }
    }
}

/// A [`SpecSet`] that is reloaded only when the user sources directory changes (a file added,
/// removed or modified), so the receiver doesn't parse YAML on every hook call. Thread-safe.
///
/// A change is noticed by a fingerprint of the directory (each spec file's name, mtime and size): a
/// `read_dir` and one `stat` per user spec per call, never a read or a parse.
pub struct SpecCache {
    settings: Settings,
    current: RwLock<Option<Cached>>,
    loads: AtomicU64,
}

impl std::fmt::Debug for SpecCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpecCache").field("loads", &self.loads).finish_non_exhaustive()
    }
}

struct Cached {
    fingerprint: Fingerprint,
    set: Arc<SpecSet>,
}

/// (name, mtime, size) of every user spec file, sorted; `None` when the directory can't be read.
type Fingerprint = Option<Vec<(String, Option<SystemTime>, u64)>>;

impl SpecCache {
    pub fn new(settings: Settings) -> Self {
        SpecCache { settings, current: RwLock::new(None), loads: AtomicU64::new(0) }
    }

    /// The current set (reloaded first if a user spec changed).
    pub fn get(&self) -> Arc<SpecSet> {
        let fingerprint = fingerprint(&self.settings.user_sources());
        if let Some(set) = self.cached(&fingerprint) {
            return set;
        }
        // The cache only ever holds a whole (fingerprint, set) pair, so a poisoned lock is still valid.
        let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = current.as_ref().filter(|c| c.fingerprint == fingerprint) {
            return Arc::clone(&cached.set); // another thread reloaded it meanwhile
        }
        let set = Arc::new(load_spec_set(&self.settings));
        self.loads.fetch_add(1, Ordering::Relaxed);
        *current = Some(Cached { fingerprint, set: Arc::clone(&set) });
        set
    }

    /// How many times the set was (re)loaded from disk.
    pub fn loads(&self) -> u64 {
        self.loads.load(Ordering::Relaxed)
    }

    fn cached(&self, fingerprint: &Fingerprint) -> Option<Arc<SpecSet>> {
        let current = self.current.read().unwrap_or_else(PoisonError::into_inner);
        current.as_ref().filter(|c| &c.fingerprint == fingerprint).map(|c| Arc::clone(&c.set))
    }
}

fn fingerprint(dir: &Path) -> Fingerprint {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut files: Vec<(String, Option<SystemTime>, u64)> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok().filter(|name| is_user_spec_name(name))?;
            // Follows symlinks, as loading does.
            let meta = std::fs::metadata(entry.path()).ok()?;
            Some((name, meta.modified().ok(), meta.len()))
        })
        .collect();
    files.sort_unstable();
    Some(files)
}
