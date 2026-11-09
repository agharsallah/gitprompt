//! Source specs: everything GitPrompt knows about a tool lives in a YAML file, not in code.
//!
//! Bundled specs are compiled into the binary ([`BUNDLED`]); a file with the same name in
//! `<home>/sources/` overrides one. Each file loads on its own: a broken file is reported with its
//! error and skipped, and a broken override of a bundled spec falls back to the bundled file
//! (ADR-0017). Port of `sources/spec.py`, plus [`SpecCache`] (reloads only when a user spec file
//! changes, instead of re-reading YAML on every request).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::json::Mapping;
use crate::{Json, JsonMap, Result, Settings};

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

/// A regular expression from a spec, with Python `re` features specs rely on (look-around).
/// Compares and serializes as its source text.
#[derive(Clone, Debug)]
pub struct Pattern(pub fancy_regex::Regex);

impl Pattern {
    pub fn new(_source: &str) -> Result<Self> {
        todo!()
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    /// Whether the pattern matches at the very start of `text` (Python `re.match`).
    pub fn matches_start(&self, _text: &str) -> bool {
        todo!()
    }
    /// The first capture group of every match (`tool_files`).
    pub fn first_groups(&self, _text: &str) -> Vec<String> {
        todo!()
    }
    /// Whether the pattern matches anywhere in `text` (Python `re.search`), e.g. a [`Spec::marker`]
    /// against a hook command line. A regex runtime error (backtrack limit) counts as no match.
    pub fn is_match(&self, _text: &str) -> bool {
        todo!()
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let source = String::deserialize(d)?;
        Pattern::new(&source).map_err(serde::de::Error::custom)
    }
}

/// How a source's request bodies are decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Json,
    Otlp,
}

/// Who produced a prompt's text (ADR-0010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Typed,
    SlashCommand,
    TaskNotification,
    AgentMessage,
    Subagent,
    Programmatic,
    Unknown,
}

/// What a connector can feed into the dashboard (docs/sources.md "Capabilities per tool").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Prompt,
    Cwd,
    Model,
    Usage,
    Cost,
    TurnEnd,
    Answer,
    Tools,
    Subagents,
    OriginMarkers,
    TranscriptPath,
    SessionLifecycle,
    SessionMetrics,
}

/// How events that are not prompts find their prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Join {
    /// By the mapped `prompt_id`, which the tool sends on every event.
    #[default]
    PromptId,
    /// The tool sends no prompt id: prompts are named `<source>#<row id>`, other events join the
    /// latest prompt of their session received before them.
    Session,
}

/// What a tool call does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Read,
    Write,
    Shell,
    Web,
    Mcp,
    Agent,
    #[default]
    Other,
}

/// What one model call reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageValue {
    Model,
    InputTokens,
    OutputTokens,
    CacheReadTokens,
    CacheCreationTokens,
    ReasoningTokens,
    CostUsd,
    DurationMs,
    QuerySource,
    Error,
    Status,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Usage {
    pub events: Vec<String>,
    pub values: IndexMap<UsageValue, String>,
    /// The tool counts cached input in its input tokens; GitPrompt's input tokens leave them out.
    pub input_includes_cached: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Install {
    /// Config file the tool reads hooks from (`${VAR:-default}` and `~` expanded).
    pub file: String,
    /// JSON merged into it; strings may use `{{receiver}}`, `{{source}}`, `{{token}}`.
    pub merge: JsonMap,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub files: Vec<String>,
    pub block_if: IndexMap<String, Vec<Json>>,
    pub block_if_set: Vec<String>,
    pub warn_if_set: Vec<String>,
    pub warn_env: Vec<String>,
}

/// Conditions of an origin or surface rule; every condition given must hold. At least one
/// condition besides `equals` is required, and `equals` needs `path`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct When {
    pub event: Option<String>,
    pub field: Option<String>,
    pub path: Option<String>,
    pub equals: Option<Json>,
    pub text_prefix: Option<String>,
    pub text_regex: Option<Pattern>,
}

/// The tool's event `from` is stored under GitPrompt's name `to`, when the payload has `equals` at
/// `path` (if given).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventName {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub equals: Option<Json>,
}

impl EventName {
    pub fn applies(&self, _event: Option<&str>, _payload: &Json) -> bool {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginRule {
    pub when: When,
    pub origin: Origin,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceRule {
    pub when: When,
    /// `^[a-z][a-z0-9-]*$`
    pub surface: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceInfo {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_icon() -> String {
    "app".into()
}

/// `{id, name, icon}` of a surface, for a prompt's facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRef {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub assistant: String,
    #[serde(default)]
    pub short_name: String,
    #[serde(default)]
    pub part_of: Option<String>,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub summary: String,
    pub detect: Vec<String>,
    #[serde(default)]
    pub version_cmd: Vec<String>,
    pub install: Install,
    #[serde(default)]
    pub format: Format,
    #[serde(default = "default_prompt_events")]
    pub prompt_events: Vec<String>,
    #[serde(default)]
    pub accept_events: Vec<String>,
    #[serde(default)]
    pub drop_events: IndexMap<String, String>,
    #[serde(default)]
    pub map: Mapping,
    #[serde(default)]
    pub event_names: Vec<EventName>,
    #[serde(default)]
    pub answer: IndexMap<String, String>,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default)]
    pub terms: String,
    #[serde(default)]
    pub after_connect: String,
    #[serde(default)]
    pub documented: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub not_captured: IndexMap<String, String>,
    #[serde(default)]
    pub origin: Vec<OriginRule>,
    #[serde(default)]
    pub injected: Vec<String>,
    #[serde(default)]
    pub surface: Vec<SurfaceRule>,
    #[serde(default)]
    pub surfaces: IndexMap<String, SurfaceInfo>,
    #[serde(default)]
    pub captures: IndexMap<String, String>,
    #[serde(default)]
    pub cannot_capture: IndexMap<String, String>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub capability_since: IndexMap<Capability, String>,
    #[serde(default)]
    pub join: Join,
    #[serde(default)]
    pub tool_kinds: IndexMap<String, ToolKind>,
    #[serde(default)]
    pub tool_files: IndexMap<String, Pattern>,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default)]
    pub shown: IndexMap<String, String>,
    #[serde(default)]
    pub ignored: IndexMap<String, String>,
    #[serde(default)]
    pub discard: IndexMap<String, String>,
}

fn default_prompt_events() -> Vec<String> {
    vec!["UserPromptSubmit".into()]
}

impl Spec {
    /// `{id, name, icon}` of a surface; `None` when unknown or not named in `surfaces`.
    pub fn surface_info(&self, _surface: Option<&str>) -> Option<SurfaceRef> {
        todo!()
    }

    /// The mapped fields of a payload (`map`), with its event under GitPrompt's name (`event_names`,
    /// first applicable rule wins).
    pub fn extract(&self, _payload: &Json) -> JsonMap {
        todo!()
    }

    /// The payload without the top-level `discard` keys (unchanged when `discard` is empty or the
    /// payload is not an object).
    pub fn discarding(&self, _payload: Json) -> Json {
        todo!()
    }

    /// The tool's name on a prompt's badge: `short_name`, else `name`.
    pub fn label(&self) -> &str {
        todo!()
    }

    /// Whether an incoming event is stored: in `accept_events` (when non-empty), and not matched by
    /// any `drop_events` pattern. `None` (no event name) is kept unless `accept_events` is set.
    pub fn keeps(&self, _event: Option<&str>) -> bool {
        todo!()
    }

    /// Matches this source's receiver URL (`/ingest/<id>`), not one of a source whose id merely
    /// starts the same (`/ingest/<id>-otel`).
    pub fn marker(&self) -> Pattern {
        todo!()
    }

    /// The kind of a tool (`tool_kinds`); unmapped = `Other`.
    pub fn tool_kind(&self, _tool: Option<&str>) -> ToolKind {
        todo!()
    }
}

/// A table entry by tool name: the exact name, else the first matching `prefix*` in table order.
pub fn by_name<'a, T>(_table: &'a IndexMap<String, T>, _name: Option<&str>) -> Option<&'a T> {
    todo!()
}

/// An event (or tool) name pattern: exact, or a prefix when it ends with `*`. `None` never matches.
pub fn matches(_pattern: &str, _name: Option<&str>) -> bool {
    todo!()
}

/// `${VAR:-default}` expansion from `settings.env`, then `~` resolved against `settings.user_home`.
pub fn expand(_path: &str, _settings: &Settings) -> PathBuf {
    todo!()
}

/// One spec file on top of the shared defaults. Merge rules: `map`, `tool_kinds` replace defaults per
/// key; `origin`, `surface` = the spec's own rules then the defaults'; `injected` = defaults then own,
/// deduplicated; `usage` and `answer` replace the defaults whole; `shown`, `ignored`, `surfaces` add to
/// the defaults' keys. Every surface named by a `surface` rule or `captures` must be in `surfaces`.
/// Errors are `Error::Spec` naming the file ("is not valid YAML", "must be a YAML mapping",
/// "is invalid").
pub fn parse_spec(_name: &str, _text: &str, _defaults: &serde_norway::Mapping) -> Result<Spec> {
    todo!()
}

/// The bundled `_defaults.yaml`, parsed.
pub fn bundled_defaults() -> serde_norway::Mapping {
    todo!()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn get(&self, _id: &str) -> Option<&Arc<Spec>> {
        todo!()
    }
}

/// The text of a spec file (bundled contents, or the user file read from disk).
pub fn read_spec_file(_file: &SpecFile) -> Result<String> {
    todo!()
}

/// Every usable spec, the file each came from, and the files that can't be used. Never fails over a
/// spec file: an unreadable/invalid one becomes a problem (a user override falls back to the bundled
/// spec of the same name), a duplicate id is a problem, a `part_of` naming a missing or non-main spec
/// is a problem. `_defaults.yaml` in the user directory is ignored (not overridable).
pub fn load_spec_set(_settings: &Settings) -> SpecSet {
    todo!()
}

/// The usable specs by id.
pub fn load_specs(_settings: &Settings) -> BTreeMap<String, Arc<Spec>> {
    todo!()
}

/// A [`SpecSet`] that is reloaded only when the user sources directory changes (a file added,
/// removed or modified), so the receiver doesn't parse YAML on every hook call. Thread-safe.
pub struct SpecCache {
    _settings: Settings,
}

impl SpecCache {
    pub fn new(settings: Settings) -> Self {
        SpecCache { _settings: settings }
    }

    /// The current set (reloaded first if a user spec changed).
    pub fn get(&self) -> Arc<SpecSet> {
        todo!()
    }

    /// How many times the set was (re)loaded from disk.
    pub fn loads(&self) -> u64 {
        todo!()
    }
}
