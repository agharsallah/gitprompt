//! What a spec says about a tool: the [`Spec`] itself and the rules and tables it holds.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Pattern;
use crate::json::{Mapping, pluck};
use crate::{Json, JsonMap};

/// How a source's request bodies are decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Json,
    Otlp,
}

/// Who produced a prompt's text (ADR-0010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Surface")]
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
