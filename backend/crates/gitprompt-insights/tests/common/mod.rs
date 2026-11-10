//! Shared builders for the insights tests: bundled specs loaded from temp dirs, prompt and event
//! rows with sensible defaults, story rows, commits, fixture replay and a timezone with daylight
//! saving. Each test binary uses part of them.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jiff::civil::date;
use jiff::tz::{Offset, TimeZone};
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use gitprompt_core::git::Commit;
use gitprompt_core::spec::{Spec, SurfaceRef, load_specs, parse_spec};
use gitprompt_core::{Json, JsonMap, Settings};
use gitprompt_insights::Specs;
use gitprompt_insights::facts::{Connector, FactRow, Facts};
use gitprompt_insights::zone::Zone;
use gitprompt_store::index::{IndexRow, UsageTotals};
use gitprompt_store::timeline::IndexedPrompt;
use gitprompt_store::{Annotation, EventRow, Outcome, PromptRow, PromptView, Store};

// --- specs -------------------------------------------------------------------------------------

/// The bundled specs, loaded with settings rooted in an empty temp dir (no user overrides).
pub fn specs() -> Specs {
    let dir = tempfile::tempdir().expect("temp dir");
    load_specs(&Settings::new(dir.path().join("gitprompt"), dir.path().join("home")))
}

pub fn spec(id: &str) -> Arc<Spec> {
    specs().get(id).cloned().unwrap_or_else(|| panic!("no bundled spec {id}"))
}

/// The text of a bundled spec file.
pub fn bundled(name: &str) -> &'static str {
    gitprompt_core::spec::BUNDLED.iter().find(|(n, _)| *n == name).map(|(_, text)| *text).expect("bundled spec")
}

/// The specs with one user override file (written to the user sources dir of a temp home).
pub fn specs_with_override(file: &str, text: &str) -> Specs {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = Settings::new(dir.path().join("gitprompt"), dir.path().join("home"));
    fs::create_dir_all(settings.user_sources()).expect("user sources dir");
    fs::write(settings.user_sources().join(file), text).expect("override written");
    load_specs(&settings)
}

/// A spec parsed on top of the bundled defaults.
pub fn custom_spec(yaml: &str) -> Spec {
    parse_spec("t.yaml", yaml, &gitprompt_core::spec::bundled_defaults()).expect("valid spec")
}

/// The minimal YAML every spec needs, followed by `rest`.
pub fn spec_yaml(rest: &str) -> String {
    format!("id: t\nname: T\ndetect: []\ninstall: {{file: x, merge: {{}}}}\n{rest}")
}

/// A copy of a spec with its origin rules replaced.
pub fn with_origin_rules(spec: &Spec, rules: Vec<gitprompt_core::spec::OriginRule>) -> Spec {
    Spec { origin: rules, ..spec.clone() }
}

pub fn surface_ref(id: &str, name: &str, icon: &str) -> SurfaceRef {
    SurfaceRef { id: id.into(), name: name.into(), icon: icon.into() }
}

// --- prompts and events ------------------------------------------------------------------------

pub const PROMPT_AT: &str = "2026-11-03T10:00:00+00:00";
pub const EVENT_AT: &str = "2026-11-03T10:00:30+00:00";

pub fn obj(value: Value) -> JsonMap {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

/// A stored prompt as `prompt_view` returns it.
#[derive(Clone, Debug)]
pub struct PromptBuilder {
    view: PromptView,
}

/// A Claude Code prompt with this text (in `fields.prompt` and `raw.prompt`), id 1, turn 1.
pub fn prompt(text: &str) -> PromptBuilder {
    PromptBuilder {
        view: PromptView {
            row: PromptRow {
                id: 1,
                source: "claude-code".into(),
                received_at: PROMPT_AT.into(),
                prompt: Some(text.into()),
                fields: obj(json!({"prompt": text})),
                context: JsonMap::new(),
                raw: json!({"prompt": text}),
            },
            annotation: Annotation::default(),
            turn: 1,
            prev_at: None,
            next_head: None,
        },
    }
}

impl PromptBuilder {
    pub fn id(mut self, id: i64) -> Self {
        self.view.row.id = id;
        self
    }
    pub fn source(mut self, source: &str) -> Self {
        self.view.row.source = source.into();
        self
    }
    pub fn at(mut self, at: &str) -> Self {
        self.view.row.received_at = at.into();
        self
    }
    pub fn turn(mut self, turn: i64) -> Self {
        self.view.turn = turn;
        self
    }
    pub fn prev_at(mut self, at: &str) -> Self {
        self.view.prev_at = Some(at.into());
        self
    }
    pub fn field(mut self, key: &str, value: Json) -> Self {
        self.view.row.fields.insert(key.into(), value);
        self
    }
    /// Sets a key of the raw payload.
    pub fn raw(mut self, key: &str, value: Json) -> Self {
        self.view.row.raw.as_object_mut().expect("raw object").insert(key.into(), value);
        self
    }
    pub fn context(mut self, key: &str, value: Json) -> Self {
        self.view.row.context.insert(key.into(), value);
        self
    }
    pub fn no_text(mut self) -> Self {
        self.view.row.prompt = None;
        self.view.row.fields.remove("prompt");
        self.view.row.raw = json!({});
        self
    }
    pub fn build(self) -> PromptView {
        self.view
    }
    pub fn row(self) -> PromptRow {
        self.view.row
    }
}

/// An event of the turn, received at [`EVENT_AT`].
pub fn event(name: &str, raw: Json) -> EventRow {
    event_at(name, EVENT_AT, raw)
}

pub fn event_at(name: &str, at: &str, raw: Json) -> EventRow {
    EventRow {
        id: 1,
        source: "claude-code".into(),
        received_at: at.into(),
        event: Some(name.into()),
        session_id: None,
        prompt_id: None,
        fields: JsonMap::new(),
        raw,
    }
}

/// A telemetry `user_prompt` event with these resource attributes.
pub fn telemetry(attrs: Json) -> EventRow {
    let mut raw = obj(json!({"event.name": "user_prompt"}));
    raw.extend(obj(attrs));
    event("user_prompt", Value::Object(raw))
}

/// The same events, all received at `at`.
pub fn all_at(events: Vec<EventRow>, at: &str) -> Vec<EventRow> {
    events.into_iter().map(|e| EventRow { received_at: at.into(), ..e }).collect()
}

// --- index rows and story rows -------------------------------------------------------------------

/// An index row of `source` with nothing reported.
pub fn index_row(source: &str) -> IndexRow {
    IndexRow {
        id: 1,
        source: source.into(),
        received_at: PROMPT_AT.into(),
        day: "2026-11-03".into(),
        hour: 10,
        weekday: 2,
        origin: "typed".into(),
        ..IndexRow::default()
    }
}

pub fn usage(api_requests: i64, input: i64, output: i64, cache_read: i64, cost: f64) -> UsageTotals {
    UsageTotals {
        api_requests,
        cost_usd: cost,
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_creation_tokens: 0,
    }
}

pub const T0: &str = "2026-03-02T09:00:00+00:00";

/// T0 plus some minutes, ISO-8601 like Python's `isoformat` (`+00:00`, no fraction when whole).
pub fn at(minutes: f64) -> String {
    let t0: Timestamp = T0.parse().expect("T0");
    let t = (t0 + SignedDuration::from_millis((minutes * 60_000.0).round() as i64)).to_zoned(TimeZone::UTC);
    let format = if t.subsec_nanosecond() == 0 { "%Y-%m-%dT%H:%M:%S%:z" } else { "%Y-%m-%dT%H:%M:%S%.3f%:z" };
    t.strftime(format).to_string()
}

/// A prompt as the timeline returns it, with defaults like the Python tests' `row()`: session s1,
/// repo /r/app (acme/app), branch main, typed, done, a one-minute turn costing $0.50.
#[derive(Clone, Debug)]
pub struct StoryBuilder {
    row: FactRow,
}

pub fn story(id: i64, minutes: f64, text: &str) -> StoryBuilder {
    let row = IndexRow {
        id,
        prompt_id: Some(format!("p{id}")),
        source: "claude-code".into(),
        session: Some("s1".into()),
        repo: Some("/r/app".into()),
        repo_name: Some("acme/app".into()),
        branch: Some("main".into()),
        title: None,
        received_at: at(minutes),
        origin: "typed".into(),
        typed_text: Some(text.into()),
        chars: text.chars().count() as i64,
        text_hash: Some(text.to_lowercase()),
        usage: usage(1, 10, 5, 0, 0.5),
        response_s: Some(60.0),
        ended_by: Some("Stop".into()),
        tool_calls: 2,
        files_written: 1,
        ..IndexRow::default()
    };
    StoryBuilder { row: FactRow { prompt: IndexedPrompt { row, outcome: Outcome::Done }, facts: None } }
}

impl StoryBuilder {
    /// Change any index value: `.with(|r| r.mentions = 1)`.
    pub fn with(mut self, change: impl FnOnce(&mut IndexRow)) -> Self {
        change(&mut self.row.prompt.row);
        self
    }
    pub fn session(self, session: Option<&str>) -> Self {
        self.with(|r| r.session = session.map(Into::into))
    }
    pub fn branch(self, branch: Option<&str>) -> Self {
        self.with(|r| r.branch = branch.map(Into::into))
    }
    pub fn origin(self, origin: &str) -> Self {
        self.with(|r| r.origin = origin.into())
    }
    pub fn response_s(self, seconds: Option<f64>) -> Self {
        self.with(|r| r.response_s = seconds)
    }
    pub fn outcome(mut self, outcome: Outcome) -> Self {
        self.row.prompt.outcome = outcome;
        self
    }
    pub fn facts(mut self, facts: Facts) -> Self {
        self.row.facts = Some(facts);
        self
    }
    pub fn build(self) -> FactRow {
        self.row
    }
}

/// Facts with a connector, a model and a surface; nothing else reported.
pub fn some_facts(source: &str, model: Option<&str>, surface: Option<SurfaceRef>) -> Facts {
    Facts {
        connector: Connector { id: source.into(), name: source.into(), label: source.into(), assistant: String::new() },
        surface,
        model: model.map(Into::into),
        models: model.map(|m| vec![m.to_string()]).unwrap_or_default(),
        model_source: None,
        tokens: None,
        cost_usd: None,
        api_requests: 0,
        outcome: Some(Outcome::Done),
        permission_mode: None,
        tool_version: None,
        response_s: None,
        answered: false,
        tool_calls: 0,
        tool_failures: 0,
        files_written: 0,
        git_commits: 0,
        subagents: 0,
    }
}

pub fn commit(hash: &str, time: &str) -> Commit {
    Commit { hash: hash.into(), subject: hash.into(), author: "t".into(), time: time.into(), ..Commit::default() }
}

pub fn commit_with_stat(hash: &str, time: &str, files: u64, insertions: u64, deletions: u64) -> Commit {
    Commit { files, insertions, deletions, ..commit(hash, time) }
}

pub fn ids(rows: &[FactRow]) -> Vec<i64> {
    rows.iter().map(|r| r.prompt.row.id).collect()
}

// --- timezones -----------------------------------------------------------------------------------

pub fn utc() -> Offset {
    Offset::UTC
}

pub fn offset_hours(hours: i32) -> Offset {
    Offset::from_seconds(hours * 3600).expect("offset")
}

/// Central European time in 2026 (a test double for a zone database): +02:00 from 2026-03-29T01:00Z
/// to 2026-10-25T01:00Z, +01:00 otherwise.
#[derive(Clone, Copy, Debug)]
pub struct Berlin2026;

impl Zone for Berlin2026 {
    fn offset_at(&self, instant: Timestamp) -> Offset {
        let summer_from = date(2026, 3, 29).at(1, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp();
        let summer_until = date(2026, 10, 25).at(1, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp();
        let summer = instant >= summer_from && instant < summer_until;
        offset_hours(if summer { 2 } else { 1 })
    }

    fn name(&self) -> String {
        "Test/Berlin-2026".into()
    }
}

// --- connector fixtures --------------------------------------------------------------------------

pub fn fixtures_dir() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/fixtures")).to_path_buf()
}

/// The connectors that have fixtures (folder names), sorted.
pub fn fixture_tools() -> Vec<String> {
    let mut tools: Vec<String> = fs::read_dir(fixtures_dir())
        .expect("fixtures dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.is_dir())
        .map(|p| p.file_name().expect("name").to_string_lossy().into_owned())
        .collect();
    tools.sort();
    tools
}

/// One fixture: `{about, payload, expect}`.
#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub payload: Json,
    pub expect: Json,
}

/// The fixtures of a connector, in file-name order.
pub fn cases(tool: &str) -> Vec<Case> {
    let mut paths: Vec<PathBuf> = fs::read_dir(fixtures_dir().join(tool))
        .expect("tool fixtures")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let data: Json = serde_json::from_str(&fs::read_to_string(&p).expect("fixture")).expect("fixture json");
            Case {
                name: p.file_stem().expect("stem").to_string_lossy().into_owned(),
                payload: data["payload"].clone(),
                expect: data["expect"].clone(),
            }
        })
        .collect()
}

/// The rows a connector's fixtures become, built the way capture stores them (the spec's
/// `discard` and `map`), without the capture pipeline: prompts get their mapped `prompt_id` (or the
/// `<source>#<id>` name of a `join: session` tool), and each event joins the prompt its fixture's
/// `expect.turn` names (none when null). Fixture `i` is received at 10:00:`i` UTC on 2026-11-07.
#[derive(Clone, Debug, Default)]
pub struct Replayed {
    /// (fixture name, prompt)
    pub prompts: Vec<(String, PromptView)>,
    /// (fixture name, event)
    pub events: Vec<(String, EventRow)>,
}

pub fn replay(spec: &Spec, tool: &str) -> Replayed {
    let mut out = Replayed::default();
    for (i, case) in cases(tool).into_iter().enumerate() {
        let id = i as i64 + 1;
        let received_at = format!("2026-11-07T10:00:{:02}+00:00", i);
        let raw = spec.discarding(case.payload.clone());
        let mut fields = spec.extract(&raw);
        let text = |key: &str, fields: &JsonMap| fields.get(key).and_then(Json::as_str).map(String::from);
        match case.expect["stored_as"].as_str() {
            Some("prompt") => {
                let pid = text("prompt_id", &fields).unwrap_or_else(|| Store::turn_name(tool, id));
                fields.insert("prompt_id".into(), json!(pid));
                let row = PromptRow {
                    id,
                    source: tool.into(),
                    received_at,
                    prompt: text("prompt", &fields),
                    fields,
                    context: JsonMap::new(),
                    raw,
                };
                out.prompts
                    .push((case.name, PromptView { row, annotation: Annotation::default(), turn: 1, prev_at: None, next_head: None }));
            }
            Some("event") => {
                let joined =
                    case.expect["turn"].as_str().map(|turn| out.prompt(turn).row.fields["prompt_id"].as_str().expect("pid").to_string());
                let row = EventRow {
                    id,
                    source: tool.into(),
                    received_at,
                    event: text("event", &fields),
                    session_id: text("session_id", &fields),
                    prompt_id: joined,
                    fields,
                    raw,
                };
                out.events.push((case.name, row));
            }
            _ => {}
        }
    }
    out
}

impl Replayed {
    pub fn prompt(&self, fixture: &str) -> &PromptView {
        &self.prompts.iter().find(|(name, _)| name == fixture).unwrap_or_else(|| panic!("no prompt fixture {fixture}")).1
    }

    /// The events joined to a prompt fixture, in arrival order.
    pub fn turn(&self, fixture: &str) -> Vec<EventRow> {
        let pid = self.prompt(fixture).row.fields.get("prompt_id").cloned();
        self.events.iter().filter(|(_, e)| e.prompt_id.as_ref().map(|p| json!(p)) == pid).map(|(_, e)| e.clone()).collect()
    }

    pub fn all_events(&self) -> Vec<EventRow> {
        self.events.iter().map(|(_, e)| e.clone()).collect()
    }

    pub fn all_prompts(&self) -> Vec<PromptView> {
        self.prompts.iter().map(|(_, p)| p.clone()).collect()
    }
}
