//! Source specs: everything GitPrompt knows about a tool lives in a YAML file, not in code.
//!
//! Bundled specs are compiled into the binary ([`BUNDLED`]); a file with the same name in
//! `<home>/sources/` overrides one. Each file loads on its own: a broken file is reported with its
//! error and skipped, and a broken override of a bundled spec falls back to the bundled file
//! (ADR-0017). Port of `sources/spec.py`, plus [`SpecCache`] (reloads only when a user spec file
//! changes, instead of re-reading YAML on every request).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};
use std::time::SystemTime;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_norway::{Mapping as YamlMap, Value as Yaml};

use crate::json::{Mapping, pluck};
use crate::settings::expand_tilde;
use crate::{Error, Json, JsonMap, Result, Settings};

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
    /// Compile `source`; an invalid pattern is an `Error::Spec`.
    pub fn new(source: &str) -> Result<Self> {
        fancy_regex::Regex::new(source).map(Pattern).map_err(|e| Error::Spec(format!("invalid regular expression {source:?}: {e}")))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    /// Whether the pattern matches at the very start of `text` (Python `re.match`).
    /// A regex runtime error counts as no match.
    pub fn matches_start(&self, text: &str) -> bool {
        // The leftmost match starts at 0 whenever any match does.
        match self.0.find(text) {
            Ok(found) => found.is_some_and(|m| m.start() == 0),
            Err(e) => self.failed(&e),
        }
    }
    /// The first capture group of every match (`tool_files`).
    /// Matches without a first group are skipped; a regex runtime error ends the list.
    pub fn first_groups(&self, text: &str) -> Vec<String> {
        self.0
            .captures_iter(text)
            .map_while(|captures| captures.map_err(|e| self.failed(&e)).ok())
            .filter_map(|captures| captures.get(1).map(|group| group.as_str().to_owned()))
            .collect()
    }
    /// Whether the pattern matches anywhere in `text` (Python `re.search`), e.g. a [`Spec::marker`]
    /// against a hook command line. A regex runtime error (backtrack limit) counts as no match.
    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text).unwrap_or_else(|e| self.failed(&e))
    }

    /// Log a regex runtime error; the search counts as finding nothing.
    fn failed(&self, error: &fancy_regex::Error) -> bool {
        tracing::warn!(pattern = self.as_str(), %error, "pattern failed at run time; treated as no match");
        false
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
    /// What the tool expects the hook to print, by the tool's own event name (`*` = any other event);
    /// none = print nothing. The `gitprompt hook` forwarder prints it (Cursor: `{"continue":true}`
    /// before a prompt is submitted, `{}` otherwise).
    #[serde(default)]
    pub reply: IndexMap<String, Json>,
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
/// condition besides `equals` is required, `equals` needs `path`, and `text_block` names at least
/// one tag (each like an `injected` tag).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct When {
    pub event: Option<String>,
    pub field: Option<String>,
    pub path: Option<String>,
    pub equals: Option<Json>,
    pub text_prefix: Option<String>,
    pub text_regex: Option<Pattern>,
    /// The typed text opens with a whole `<tag ...>...</tag>` block of one of these tags: how a tool
    /// wraps a message it sends on its own (`<task-notification>`, `<agent-message from="...">`).
    pub text_block: Option<Vec<String>>,
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
    /// Whether this rule renames `event` in `payload`. A missing (or null) value at `path` equals
    /// only an `equals` that is not given; numbers compare by value (`1` = `1.0`).
    pub fn applies(&self, event: Option<&str>, payload: &Json) -> bool {
        if event != Some(self.from.as_str()) {
            return false;
        }
        let Some(path) = &self.path else { return true };
        match (pluck(payload, path).filter(|v| !v.is_null()), &self.equals) {
            (None, None) => true,
            (Some(found), Some(expected)) => same_value(found, expected),
            _ => false,
        }
    }
}

/// JSON equality, with numbers compared by value.
fn same_value(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        _ => a == b,
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
    pub fn surface_info(&self, surface: Option<&str>) -> Option<SurfaceRef> {
        let id = surface.filter(|id| !id.is_empty())?;
        let info = self.surfaces.get(id)?;
        Some(SurfaceRef { id: id.to_owned(), name: info.name.clone(), icon: info.icon.clone() })
    }

    /// The mapped fields of a payload (`map`), with its event under GitPrompt's name (`event_names`,
    /// first applicable rule wins).
    pub fn extract(&self, payload: &Json) -> JsonMap {
        let mut fields = crate::json::extract(payload, &self.map);
        let event = fields.get("event").and_then(Json::as_str);
        if let Some(rule) = self.event_names.iter().find(|rule| rule.applies(event, payload)) {
            fields.insert("event".into(), Json::String(rule.to.clone()));
        }
        fields
    }

    /// The payload without the top-level `discard` keys (unchanged when `discard` is empty or the
    /// payload is not an object).
    pub fn discarding(&self, payload: Json) -> Json {
        match payload {
            Json::Object(mut map) if !self.discard.is_empty() => {
                map.retain(|key, _| !self.discard.contains_key(key));
                Json::Object(map)
            }
            other => other,
        }
    }

    /// The tool's name on a prompt's badge: `short_name`, else `name`.
    pub fn label(&self) -> &str {
        if self.short_name.is_empty() { &self.name } else { &self.short_name }
    }

    /// Whether an incoming event is stored: in `accept_events` (when non-empty), and not matched by
    /// any `drop_events` pattern. `None` (no event name) is kept unless `accept_events` is set.
    pub fn keeps(&self, event: Option<&str>) -> bool {
        let accepted = self.accept_events.is_empty() || event.is_some_and(|e| self.accept_events.iter().any(|a| a == e));
        accepted && !self.drop_events.keys().any(|pattern| matches(pattern, event))
    }

    /// Matches this source's receiver URL (`/ingest/<id>`), not one of a source whose id merely
    /// starts the same (`/ingest/<id>-otel`).
    ///
    /// # Panics
    ///
    /// Never: the id is escaped, so the pattern is a literal plus a fixed look-ahead.
    #[expect(clippy::expect_used, reason = "an escaped id plus a fixed look-ahead is always a valid pattern")]
    pub fn marker(&self) -> Pattern {
        let source = format!(r"/ingest/{}(?![\w-])", fancy_regex::escape(&self.id));
        // The id is escaped, so the pattern is a literal plus a fixed look-ahead: always valid.
        Pattern::new(&source).expect("an escaped id makes a valid pattern")
    }

    /// The kind of a tool (`tool_kinds`); unmapped = `Other`.
    pub fn tool_kind(&self, tool: Option<&str>) -> ToolKind {
        by_name(&self.tool_kinds, tool).copied().unwrap_or_default()
    }
}

/// A table entry by tool name: the exact name, else the first matching `prefix*` in table order.
pub fn by_name<'a, T>(table: &'a IndexMap<String, T>, name: Option<&str>) -> Option<&'a T> {
    name.and_then(|name| table.get(name)).or_else(|| table.iter().find(|(pattern, _)| matches(pattern, name)).map(|(_, value)| value))
}

/// An event (or tool) name pattern: exact, or a prefix when it ends with `*`. `None` never matches.
pub fn matches(pattern: &str, name: Option<&str>) -> bool {
    let Some(name) = name else { return false };
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

/// `${VAR:-default}` expansion from `settings.env`, then `~` resolved against `settings.user_home`.
pub fn expand(path: &str, settings: &Settings) -> PathBuf {
    expand_tilde(&expand_vars(path, &settings.env), &settings.user_home)
}

/// Every `${VAR:-default}`: the variable's value, or `default` when it is unset or empty. Other text,
/// `${VAR}` included, is kept as written.
fn expand_vars(path: &str, env: &crate::Env) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some((name, default, len)) = parse_var(after) {
            out.push_str(env.get(name).unwrap_or(default));
            rest = &after[len..];
        } else {
            out.push_str("${");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `NAME:-default}` at the start of `text` -> (name, default, length including the `}`).
fn parse_var(text: &str) -> Option<(&str, &str, usize)> {
    let name_len = text.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    let after_name = text[name_len..].strip_prefix(":-")?;
    let default_len = after_name.find('}')?;
    (name_len > 0).then(|| (&text[..name_len], &after_name[..default_len], name_len + 2 + default_len + 1))
}

/// One spec file on top of the shared defaults. Merge rules: `map`, `tool_kinds` replace defaults per
/// key; `origin`, `surface` = the spec's own rules then the defaults'; `injected` = defaults then own,
/// deduplicated; `usage` and `answer` replace the defaults whole; `shown`, `ignored`, `surfaces` add to
/// the defaults' keys. Every surface named by a `surface` rule or `captures` must be in `surfaces`.
/// Errors are `Error::Spec` naming the file ("is not valid YAML", "must be a YAML mapping",
/// "is invalid").
pub fn parse_spec(name: &str, text: &str, defaults: &serde_norway::Mapping) -> Result<Spec> {
    let mut data = parse_mapping(name, text)?;
    merge_defaults(&mut data, defaults);
    let invalid = |why: String| Error::Spec(format!("source spec {name} is invalid: {why}"));
    let spec: Spec = serde_norway::from_value(Yaml::Mapping(data)).map_err(|e| invalid(e.to_string()))?;
    validate(&spec).map_err(invalid)?;
    Ok(spec)
}

/// The bundled `_defaults.yaml`, parsed. It is compiled in and checked by the tests; were it ever
/// broken, specs load without shared defaults (and the error is logged) rather than not at all.
pub fn bundled_defaults() -> serde_norway::Mapping {
    static DEFAULTS_MAP: LazyLock<YamlMap> = LazyLock::new(|| {
        parse_mapping(DEFAULTS, bundled_text(DEFAULTS).unwrap_or_default()).unwrap_or_else(|e| {
            tracing::error!(error = %e, "the bundled source spec defaults can't be read");
            YamlMap::new()
        })
    });
    DEFAULTS_MAP.clone()
}

fn bundled_text(name: &str) -> Option<&'static str> {
    BUNDLED.iter().find(|(bundled, _)| *bundled == name).map(|(_, text)| *text)
}

/// YAML text that must hold a mapping (anchors resolved, `<<` merge keys applied).
fn parse_mapping(name: &str, text: &str) -> Result<YamlMap> {
    let not_yaml = |e: serde_norway::Error| Error::Spec(format!("source spec {name} is not valid YAML: {e}"));
    let mut value: Yaml = serde_norway::from_str(text).map_err(not_yaml)?;
    value.apply_merge().map_err(not_yaml)?;
    match value {
        Yaml::Mapping(map) => Ok(map),
        _ => Err(Error::Spec(format!("source spec {name} must be a YAML mapping"))),
    }
}

/// The merge rules of [`parse_spec`]. A key whose value is "empty" (null, `{}`, `[]`, `""`, `false`,
/// 0) counts as absent. A value of the wrong type is kept as is, so deserializing reports it.
fn merge_defaults(data: &mut YamlMap, defaults: &YamlMap) {
    for key in ["map", "tool_kinds", "shown", "ignored", "surfaces"] {
        let merged = merge_mappings(given(defaults, key), given(data, key));
        data.insert(key.into(), merged);
    }
    for key in ["origin", "surface"] {
        let merged = concat(given(data, key), given(defaults, key), false);
        data.insert(key.into(), merged);
    }
    let injected = concat(given(defaults, "injected"), given(data, "injected"), true);
    data.insert("injected".into(), injected);
    for key in ["usage", "answer"] {
        let whole = given(data, key).or_else(|| given(defaults, key)).cloned().unwrap_or_else(|| Yaml::Mapping(YamlMap::new()));
        data.insert(key.into(), whole);
    }
}

/// The value at `key`, unless it is empty (Python truthiness, as the original port relied on).
fn given<'a>(map: &'a YamlMap, key: &str) -> Option<&'a Yaml> {
    map.get(key).filter(|value| match value {
        Yaml::Null => false,
        Yaml::Bool(b) => *b,
        Yaml::Number(n) => n.as_f64() != Some(0.0),
        Yaml::String(s) => !s.is_empty(),
        Yaml::Sequence(items) => !items.is_empty(),
        Yaml::Mapping(map) => !map.is_empty(),
        Yaml::Tagged(_) => true,
    })
}

/// `base` with `own`'s keys replacing or added (a key keeps its position in `base`).
fn merge_mappings(base: Option<&Yaml>, own: Option<&Yaml>) -> Yaml {
    let mut merged = match base {
        Some(Yaml::Mapping(base)) => base.clone(),
        _ => YamlMap::new(),
    };
    match own {
        Some(Yaml::Mapping(own)) => merged.extend(own.iter().map(|(k, v)| (k.clone(), v.clone()))),
        Some(other) => return other.clone(),
        None => {}
    }
    Yaml::Mapping(merged)
}

/// `first`'s items then `second`'s (without repeats when `dedup`).
fn concat(first: Option<&Yaml>, second: Option<&Yaml>, dedup: bool) -> Yaml {
    let mut items: Vec<Yaml> = Vec::new();
    for part in [first, second].into_iter().flatten() {
        let Yaml::Sequence(part) = part else { return part.clone() };
        for item in part {
            if !(dedup && items.contains(item)) {
                items.push(item.clone());
            }
        }
    }
    Yaml::Sequence(items)
}

/// What deserializing can't check: rule conditions, surface ids and names, injected tags.
fn validate(spec: &Spec) -> std::result::Result<(), String> {
    let whens = spec.origin.iter().map(|r| &r.when).chain(spec.surface.iter().map(|r| &r.when));
    for when in whens {
        check_when(when)?;
    }
    for rule in &spec.event_names {
        if rule.equals.as_ref().is_some_and(|v| !is_scalar(v)) {
            return Err(format!("event_names {:?}: `equals` must be a string, number or boolean", rule.from));
        }
    }
    let named: Vec<&str> = spec.surface.iter().map(|r| r.surface.as_str()).chain(spec.captures.keys().map(String::as_str)).collect();
    for id in spec.surfaces.keys().map(String::as_str).chain(named.iter().copied()) {
        if !is_surface_id(id) {
            return Err(format!("surface id {id:?} must match ^[a-z][a-z0-9-]*$"));
        }
    }
    let mut unknown: Vec<&str> = named.into_iter().filter(|id| !spec.surfaces.contains_key(*id)).collect();
    unknown.sort_unstable();
    unknown.dedup();
    if !unknown.is_empty() {
        return Err(format!("surfaces without a name in `surfaces`: {unknown:?}"));
    }
    match spec.injected.iter().find(|tag| !is_tag(tag)) {
        Some(tag) => Err(format!("injected tag {tag:?} must match ^[A-Za-z][\\w:-]*$")),
        None => Ok(()),
    }
}

fn check_when(when: &When) -> std::result::Result<(), String> {
    let has_condition = when.event.is_some()
        || when.field.is_some()
        || when.path.is_some()
        || when.text_prefix.is_some()
        || when.text_regex.is_some()
        || when.text_block.is_some();
    if !has_condition {
        return Err("a rule needs at least one condition".into());
    }
    match when.text_block.as_deref() {
        Some([]) => return Err("`text_block` needs at least one tag".into()),
        Some(tags) => {
            if let Some(tag) = tags.iter().find(|tag| !is_tag(tag)) {
                return Err(format!("text_block tag {tag:?} must match ^[A-Za-z][\\w:-]*$"));
            }
        }
        None => {}
    }
    match &when.equals {
        Some(_) if when.path.is_none() => Err("`equals` needs a `path`".into()),
        Some(value) if !is_scalar(value) => Err("`equals` must be a string, number or boolean".into()),
        _ => Ok(()),
    }
}

fn is_scalar(value: &Json) -> bool {
    matches!(value, Json::String(_) | Json::Number(_) | Json::Bool(_))
}

/// `^[a-z][a-z0-9-]*$`
fn is_surface_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `^[A-Za-z][\w:-]*$`
fn is_tag(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '-'))
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
    for problem in &out.problems {
        if let Some(fallback) = &problem.fallback {
            tracing::warn!("{} (using the bundled {fallback})", problem.error);
        } else {
            tracing::warn!("{}", problem.error);
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vars_without_a_default_or_unterminated_are_kept() {
        let env = crate::Env::from_pairs([("A", "x")]);
        assert_eq!(expand_vars("${A}/${A:-d}/${:-e}/${A:-", &env), "${A}/x/${:-e}/${A:-");
    }

    #[test]
    fn empty_yaml_values_count_as_absent() {
        let map: YamlMap = serde_norway::from_str("a: ~\nb: {}\nc: []\nd: ''\ne: 0\nf: false\ng: [1]\n").unwrap();
        let present: Vec<&str> = ["a", "b", "c", "d", "e", "f", "g"].into_iter().filter(|k| given(&map, k).is_some()).collect();
        assert_eq!(present, ["g"]);
    }

    #[test]
    fn surface_ids_and_tags_follow_their_patterns() {
        assert!(is_surface_id("my-app2") && !is_surface_id("App") && !is_surface_id("1app") && !is_surface_id(""));
        assert!(is_tag("ns:tag") && is_tag("A_b") && !is_tag("1abc") && !is_tag("a b") && !is_tag(""));
    }
}
