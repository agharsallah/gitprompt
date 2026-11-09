//! Shared test helpers: a temporary GitPrompt home and user home (never the real `~`), hermetic specs,
//! an in-memory store, and hand-rolled doubles for the machine (git, tool version).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitprompt_capture::pipeline::{CaptureOptions, Captured, GitProbe, Observer, StoredRow, capture};
use gitprompt_capture::probe::VersionProbe;
use gitprompt_core::spec::{Spec, load_specs};
use gitprompt_core::{Env, Json, JsonMap, Settings};
use gitprompt_store::{EventRow, PromptRow, Store};
use tempfile::TempDir;

pub const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/fixtures");

/// A temporary machine: `<tmp>/gitprompt` is GitPrompt's home, `<tmp>/home` the user's home.
pub struct TestEnv {
    pub dir: TempDir,
    pub settings: Settings,
}

impl TestEnv {
    pub fn new() -> Self {
        Self::with_env(&[])
    }

    /// Settings whose environment holds exactly these variables (never the process environment).
    pub fn with_env(vars: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut settings = Settings::new(dir.path().join("gitprompt"), dir.path().join("home"));
        settings.env = Env::from_pairs(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        fs::create_dir_all(&settings.user_home).unwrap();
        TestEnv { dir, settings }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    pub fn user_path(&self, rel: &str) -> PathBuf {
        self.settings.user_home.join(rel)
    }

    /// A bundled spec with its system-wide policy files removed (`/etc/...`, `/Library/...`), so a
    /// managed settings file on the developer's machine can't change a test. Policy files under the
    /// user's (temporary) home are kept.
    pub fn spec(&self, id: &str) -> Spec {
        let spec = load_specs(&self.settings).get(id).unwrap_or_else(|| panic!("no bundled spec {id}")).clone();
        hermetic(&spec)
    }

    /// Every bundled spec, hermetic as [`Self::spec`].
    pub fn specs(&self) -> BTreeMap<String, Arc<Spec>> {
        load_specs(&self.settings).into_iter().map(|(id, s)| (id, Arc::new(hermetic(&s)))).collect()
    }

    /// `~/.claude/settings.json` (parent created, file not).
    pub fn claude_settings(&self) -> PathBuf {
        let path = self.user_path(".claude/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        path
    }
}

pub fn hermetic(spec: &Spec) -> Spec {
    let mut spec = spec.clone();
    spec.policy.files.retain(|f| f.starts_with("${") || f.starts_with('~'));
    spec
}

// --- files -----------------------------------------------------------------------------------

pub fn write_json(path: &Path, value: &Json) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

pub fn write_text(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub fn read_json(path: &Path) -> Json {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

pub fn obj(value: Json) -> JsonMap {
    match value {
        Json::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

// --- the machine -----------------------------------------------------------------------------

/// No repository anywhere.
pub struct NoGit;

impl GitProbe for NoGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        JsonMap::new()
    }
}

/// A repository context per directory; `{}` for any other directory.
pub struct GitByDir(pub BTreeMap<PathBuf, JsonMap>);

impl GitProbe for GitByDir {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        cwd.and_then(|c| self.0.get(c)).cloned().unwrap_or_default()
    }
}

/// The installed tool reports this version (or none).
pub struct FixedVersion(pub Option<&'static str>);

impl VersionProbe for FixedVersion {
    fn tool_version(&self, _spec: &Spec) -> Option<String> {
        self.0.map(String::from)
    }
}

pub static NO_GIT: NoGit = NoGit;
pub static NO_VERSION: FixedVersion = FixedVersion(None);

/// A machine with no git and no tool version: the context of every prompt is `{}`.
pub fn blank() -> Observer<'static> {
    Observer { git: &NO_GIT, versions: &NO_VERSION }
}

// --- the store -------------------------------------------------------------------------------

pub fn store() -> Store {
    Store::open_in_memory().expect("in-memory store")
}

/// Capture with the default options (enabled, secrets kept) on a blank machine.
pub fn ingest(spec: &Spec, store: &Store, body: &Json) -> Captured {
    capture(spec, store, body, CaptureOptions::default(), &blank()).expect("capture")
}

pub fn prompt_row(store: &Store, id: i64) -> PromptRow {
    store.prompts(&[id]).unwrap().into_iter().next().unwrap_or_else(|| panic!("no prompt {id}"))
}

/// Every stored event (with an event name), oldest first.
pub fn all_events(store: &Store) -> Vec<EventRow> {
    store.events_after(0, "", &[]).unwrap()
}

pub fn event_row(store: &Store, id: i64) -> EventRow {
    all_events(store).into_iter().find(|e| e.id == id).unwrap_or_else(|| panic!("no event {id}"))
}

/// Every stored prompt, oldest first.
pub fn all_prompts(store: &Store) -> Vec<PromptRow> {
    store.prompts(&store.prompt_ids().unwrap()).unwrap()
}

/// The single row a capture stored.
pub fn only_row(captured: &Captured) -> StoredRow {
    assert_eq!(captured.rows.len(), 1, "expected exactly one stored row, got {:?}", captured.rows);
    captured.rows[0]
}

// --- fixtures --------------------------------------------------------------------------------

/// (file stem, case) of every conformance fixture of a tool, in file-name order.
pub fn fixtures(tool: &str) -> Vec<(String, Json)> {
    let dir = Path::new(FIXTURES).join(tool);
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths.into_iter().map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), read_json(&p))).collect()
}

pub fn fixture_tools() -> Vec<String> {
    let mut tools: Vec<String> = fs::read_dir(FIXTURES)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    tools.sort();
    tools
}
