//! Test data for the store tests: a prompt builder that stores the raw prompt row *and* its prompt
//! index entry (the values gitprompt-insights would derive), an event builder, and small helpers.
//! Each test states only what matters; everything else gets a sensible default.
#![allow(dead_code)]

use std::sync::Arc;

use jiff::Timestamp;
use gitprompt_core::spec::ToolKind;
use gitprompt_core::{Json, JsonMap};
use gitprompt_store::index::{Entry, IndexRow, ModelUsage, ToolCalls, UsageTotals, WorkUsage};
use gitprompt_store::rows::EventRow;
use gitprompt_store::store::Clock;
use gitprompt_store::{PromptFilter, Store};
use serde_json::json;

/// A Tuesday, 10:00 UTC.
pub const T0: &str = "2026-11-03T10:00:00.000000+00:00";

pub fn store() -> Store {
    Store::open_in_memory().expect("open an in-memory store")
}

/// A store whose "now" (the 1-hour `running` window) is pinned.
pub fn store_at(now: &str) -> Store {
    store().with_clock(fixed_clock(now))
}

pub fn fixed_clock(now: &str) -> Clock {
    let t = parse(now);
    Arc::new(move || t)
}

pub fn parse(s: &str) -> Timestamp {
    s.parse::<Timestamp>().unwrap_or_else(|e| panic!("bad timestamp {s}: {e}"))
}

/// A received_at string as the store writes them: microseconds and `+00:00`.
pub fn ts(t: Timestamp) -> String {
    t.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
}

pub fn obj(value: Json) -> JsonMap {
    match value {
        Json::Object(map) => map,
        other => panic!("not a JSON object: {other}"),
    }
}

pub fn usage(api_requests: i64, cost_usd: f64, input_tokens: i64) -> UsageTotals {
    UsageTotals { api_requests, cost_usd, input_tokens, ..UsageTotals::default() }
}

pub fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "expected {expected}, got {actual}");
}

/// Row ids the filter keeps through `search`, ascending.
pub fn found(store: &Store, filter: &PromptFilter) -> Vec<i64> {
    let mut ids: Vec<i64> = store.search(filter, 10_000).expect("search").into_iter().map(|p| p.row.id).collect();
    ids.sort_unstable();
    ids
}

/// Replace every occurrence of `from` in the strings (and keys) of a JSON value: a stand-in for
/// secret hiding in `rewrite` tests.
pub fn replacing(from: &'static str, to: &'static str) -> impl Fn(&Json) -> Json {
    move |value| serde_json::from_str(&value.to_string().replace(from, to)).expect("still JSON")
}

// --- prompts ------------------------------------------------------------------------------------

/// A prompt row plus its index entry. `insert` stores both (ids, source, prompt id, session and
/// received_at are kept consistent between them); `insert_unindexed` stores only the raw row.
#[derive(Clone)]
pub struct PromptBuilder {
    source: String,
    fields: JsonMap,
    context: JsonMap,
    raw: Option<Json>,
    at: String,
    pub entry: Entry,
}

pub fn prompt(prompt_id: &str) -> PromptBuilder {
    PromptBuilder::new().field("prompt_id", json!(prompt_id))
}

impl PromptBuilder {
    pub fn new() -> Self {
        let row = IndexRow {
            source: "claude-code".into(),
            received_at: T0.into(),
            day: "2026-11-03".into(),
            hour: 10,
            weekday: 2,
            origin: "typed".into(),
            ..IndexRow::default()
        };
        PromptBuilder {
            source: "claude-code".into(),
            fields: obj(json!({"event": "UserPromptSubmit", "session_id": "s1"})),
            context: JsonMap::new(),
            raw: None,
            at: T0.into(),
            entry: Entry { row, ..Entry::default() },
        }
        .text("hello")
    }

    pub fn source(mut self, source: &str) -> Self {
        self.source = source.into();
        self
    }

    pub fn field(mut self, key: &str, value: Json) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    pub fn without_field(mut self, key: &str) -> Self {
        self.fields.remove(key);
        self
    }

    pub fn session(self, session: &str) -> Self {
        self.field("session_id", json!(session))
    }

    pub fn no_session(self) -> Self {
        self.without_field("session_id")
    }

    /// What was sent and what was typed are the same text.
    pub fn text(self, text: &str) -> Self {
        self.field("prompt", json!(text)).typed(text)
    }

    /// The typed text only (the index's view), e.g. without injected context.
    pub fn typed(mut self, text: &str) -> Self {
        self.entry.row.typed_text = Some(text.into());
        self.entry.row.chars = text.chars().count() as i64;
        self
    }

    pub fn at(mut self, received_at: &str) -> Self {
        self.at = received_at.into();
        self
    }

    /// Local calendar day, hour and ISO weekday (as the indexer would compute them).
    pub fn local(mut self, day: &str, hour: u8, weekday: u8) -> Self {
        self.entry.row.day = day.into();
        self.entry.row.hour = hour;
        self.entry.row.weekday = weekday;
        self
    }

    pub fn origin(mut self, origin: &str) -> Self {
        self.entry.row.origin = origin.into();
        self
    }

    pub fn context(mut self, context: Json) -> Self {
        self.context = obj(context);
        self
    }

    pub fn raw(mut self, raw: Json) -> Self {
        self.raw = Some(raw);
        self
    }

    pub fn ended(mut self, by: &str, response_s: f64) -> Self {
        self.entry.row.ended_by = Some(by.into());
        self.entry.row.response_s = Some(response_s);
        self
    }

    /// A model call total of the turn; also adds to the prompt's own usage.
    pub fn model(mut self, model: &str, usage: UsageTotals) -> Self {
        let row = &mut self.entry.row;
        row.usage.api_requests += usage.api_requests;
        row.usage.cost_usd += usage.cost_usd;
        row.usage.input_tokens += usage.input_tokens;
        row.usage.output_tokens += usage.output_tokens;
        row.usage.cache_read_tokens += usage.cache_read_tokens;
        row.usage.cache_creation_tokens += usage.cache_creation_tokens;
        row.model.get_or_insert_with(|| model.to_string());
        self.entry.models.push(ModelUsage { model: Some(model.into()), usage });
        self
    }

    /// Tool calls of the turn; also adds to the prompt's tool_calls.
    pub fn tool(mut self, tool: &str, kind: ToolKind, calls: i64) -> Self {
        self.entry.row.tool_calls += calls;
        self.entry.tools.push(ToolCalls { tool: Some(tool.into()), kind, calls });
        self
    }

    pub fn work(mut self, work: &str, query_source: Option<&str>, usage: UsageTotals) -> Self {
        self.entry.work.push(WorkUsage { work: work.into(), query_source: query_source.map(Into::into), usage });
        self
    }

    /// Any other index value.
    pub fn with(mut self, edit: impl FnOnce(&mut IndexRow)) -> Self {
        edit(&mut self.entry.row);
        self
    }

    /// The index entry this prompt would get as row `id`.
    pub fn entry_for(&self, id: i64) -> Entry {
        let mut entry = self.entry.clone();
        entry.row.id = id;
        entry.row.source = self.source.clone();
        entry.row.prompt_id = self.fields.get("prompt_id").and_then(Json::as_str).map(Into::into);
        entry.row.session = self.fields.get("session_id").and_then(Json::as_str).map(Into::into);
        entry.row.received_at = self.at.clone();
        entry
    }

    pub fn insert_unindexed(&self, store: &Store) -> i64 {
        let raw = self.raw.clone().unwrap_or_else(|| Json::Object(self.fields.clone()));
        store.add_prompt_at(&self.source, &self.fields, &self.context, &raw, &self.at).expect("add prompt").expect("not a duplicate").id
    }

    pub fn insert(&self, store: &Store) -> i64 {
        let id = self.insert_unindexed(store);
        store.index().upsert(&[self.entry_for(id)]).expect("index the prompt");
        id
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// --- events -------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct EventBuilder {
    source: String,
    fields: JsonMap,
    raw: Json,
    at: String,
}

/// An event named `name` of source claude-code in session s1, received at T0 + 30 s.
pub fn event(name: &str) -> EventBuilder {
    EventBuilder {
        source: "claude-code".into(),
        fields: obj(json!({"event": name, "session_id": "s1"})),
        raw: json!({"hook_event_name": name}),
        at: "2026-11-03T10:00:30.000000+00:00".into(),
    }
}

impl EventBuilder {
    pub fn source(mut self, source: &str) -> Self {
        self.source = source.into();
        self
    }

    pub fn unnamed(mut self) -> Self {
        self.fields.remove("event");
        self
    }

    pub fn prompt_id(mut self, prompt_id: &str) -> Self {
        self.fields.insert("prompt_id".into(), json!(prompt_id));
        self
    }

    pub fn session(mut self, session: &str) -> Self {
        self.fields.insert("session_id".into(), json!(session));
        self
    }

    pub fn no_session(mut self) -> Self {
        self.fields.remove("session_id");
        self
    }

    pub fn field(mut self, key: &str, value: Json) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    pub fn raw(mut self, raw: Json) -> Self {
        self.raw = raw;
        self
    }

    pub fn at(mut self, received_at: &str) -> Self {
        self.at = received_at.into();
        self
    }

    pub fn insert(&self, store: &Store) -> EventRow {
        store.add_event_at(&self.source, &self.fields, &self.raw, &self.at).expect("add event")
    }
}
