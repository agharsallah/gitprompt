//! The capture pipeline, shared by every channel:
//!
//! ```text
//! request body --decode (spec.format)--> payloads --discard, redact--> map (spec) --keep?--> prompt or event --> store
//! ```
//!
//! Port of `capture/pipeline.py`, plus the enabled check of `api/routes/ingest.py`.
//!
//! - A turned-off source ([`CaptureOptions::enabled`] false) discards everything.
//! - The payload keys the spec lists in `discard` are removed before anything else (an account
//!   email never reaches the store); with `hide_secrets` detected secrets are replaced too.
//! - Events the spec does not keep (`accept_events`, `drop_events`) are discarded before storage.
//! - A payload whose event is one of `spec.prompt_events` becomes a prompt, stamped with what
//!   GitPrompt observes on this machine at that moment ([`observe`]); a repeated prompt id of the
//!   same source is not stored twice.
//! - Every other payload is stored as an event (long strings clipped, `core::json::clip`) and
//!   attaches to its prompt by `prompt_id`. A `join: session` source sends no prompt id: each prompt
//!   is named `<source>#<row id>`, and its other events take the id of the latest prompt of their
//!   session ([`join_session`]).
//!
//! The machine is read (git, `--version`) before anything is written; the rows of one request body
//! are then written in one store transaction, so an OTLP batch is stored whole or not at all.
//!
//! Synchronous on purpose: the store is sync, git and `--version` are blocking; the server runs
//! [`capture`] in `spawn_blocking`.

use std::collections::BTreeSet;
use std::path::Path;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use gitprompt_core::git::{GitCache, git_context};
use gitprompt_core::json::{CLIP_LIMIT, clip, is_no_value};
use gitprompt_core::preferences::Preferences;
use gitprompt_core::secrets::redact;
use gitprompt_core::spec::{Format, Join, Spec};
use gitprompt_core::{Json, JsonMap, Result};
use gitprompt_store::Store;

use crate::probe::VersionProbe;

/// Repository state of a working directory (`core::git::git_context`'s keys); `{}` outside a repo.
pub trait GitProbe: Send + Sync {
    fn context(&self, cwd: Option<&Path>) -> JsonMap;
}

/// `core::git::git_context`, uncached.
#[derive(Debug)]
pub struct SystemGit;

impl GitProbe for SystemGit {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        git_context(cwd)
    }
}

impl GitProbe for GitCache {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        GitCache::context(self, cwd)
    }
}

/// What the pipeline reads from this machine when a prompt arrives.
#[derive(Clone, Copy)]
pub struct Observer<'a> {
    pub git: &'a dyn GitProbe,
    pub versions: &'a dyn VersionProbe,
}

impl std::fmt::Debug for Observer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observer").finish_non_exhaustive()
    }
}

/// Per-request switches, from the preferences at the time of the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureOptions {
    /// `false`: the source is in `disabled_sources`; everything it sends is discarded.
    pub enabled: bool,
    /// Preference `hide_secrets_on_ingest`: replace detected secrets before storing.
    pub hide_secrets: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        CaptureOptions { enabled: true, hide_secrets: false }
    }
}

impl CaptureOptions {
    /// The options for one source under these preferences.
    pub fn for_source(preferences: &Preferences, source: &str) -> Self {
        CaptureOptions {
            enabled: !preferences.disabled_sources.iter().any(|s| s == source),
            hide_secrets: preferences.hide_secrets_on_ingest,
        }
    }
}

/// A row the pipeline stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "table", content = "id", rename_all = "lowercase")]
pub enum StoredRow {
    Prompt(i64),
    Event(i64),
}

/// What one request stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Captured {
    /// New prompts (a duplicate prompt id is not counted).
    pub prompts: u64,
    pub events: u64,
    /// `prompt_ids` that received events, so the caller can mark them for the prompt index.
    pub turns: BTreeSet<String>,
    /// The rows stored, in payload order.
    pub rows: Vec<StoredRow>,
}

/// What the receiver publishes on its event bus after a capture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub prompts: u64,
    pub events: u64,
}

impl Captured {
    /// Nothing was stored (Python's `not captured`).
    pub fn is_empty(&self) -> bool {
        self.prompts == 0 && self.events == 0
    }

    pub fn notice(&self) -> Notice {
        Notice { prompts: self.prompts, events: self.events }
    }

    /// Add what another capture stored (e.g. every line of a drained spool).
    pub fn absorb(&mut self, other: Captured) {
        self.prompts += other.prompts;
        self.events += other.events;
        self.turns.extend(other.turns);
        self.rows.extend(other.rows);
    }
}

/// The payloads of one request body: the body itself for `json` (one event per hook call), the
/// records of [`crate::otlp::records`] for `otlp`.
pub fn decode(format: Format, body: &Json) -> Vec<Json> {
    match format {
        Format::Json => vec![body.clone()],
        Format::Otlp => crate::otlp::records(body),
    }
}

/// What GitPrompt sees on this machine when a prompt arrives: the repository state of `cwd`, plus
/// `tool_version` when the tool reports one.
pub fn observe(spec: &Spec, cwd: Option<&str>, observer: &Observer<'_>) -> JsonMap {
    let mut context = observer.git.context(cwd.map(Path::new));
    if let Some(version) = observer.versions.tool_version(spec) {
        context.insert("tool_version".into(), Json::String(version));
    }
    context
}

/// `join: session`: an event without a prompt id takes the id of the latest prompt of its session.
/// Unchanged for a `prompt_id` source, an event that has a prompt id or no session id, or a session
/// without a prompt yet.
pub fn join_session(spec: &Spec, store: &Store, mut fields: JsonMap) -> Result<JsonMap> {
    if spec.join != Join::Session || has_value(&fields, "prompt_id") {
        return Ok(fields);
    }
    let Some(session) = text(&fields, "session_id") else { return Ok(fields) };
    if let Some(turn) = store.latest_turn(&spec.id, &session)? {
        fields.insert("prompt_id".into(), Json::String(turn));
    }
    Ok(fields)
}

/// Run one request body through the pipeline (see the module docs). Rows are received now.
pub fn capture(spec: &Spec, store: &Store, body: &Json, options: CaptureOptions, observer: &Observer<'_>) -> Result<Captured> {
    capture_at(spec, store, body, options, observer, None)
}

/// [`capture`], with the rows received at `received_at` instead of now (`None` = now): a payload
/// replayed from the [`crate::spool`] keeps the time it was spooled.
pub fn capture_at(
    spec: &Spec,
    store: &Store,
    body: &Json,
    options: CaptureOptions,
    observer: &Observer<'_>,
    received_at: Option<Timestamp>,
) -> Result<Captured> {
    if !options.enabled {
        return Ok(Captured::default());
    }
    let rows: Vec<NewRow> = decode(spec.format, body).into_iter().filter_map(|payload| prepare(spec, payload, options, observer)).collect();
    if rows.is_empty() {
        return Ok(Captured::default());
    }
    let received_at = received_at.map(iso);
    store.transaction(|store| {
        let mut captured = Captured::default();
        for row in rows {
            store_row(spec, store, row, received_at.as_deref(), &mut captured)?;
        }
        Ok(captured)
    })
}

/// One payload, ready to store.
enum NewRow {
    Prompt { fields: JsonMap, context: JsonMap, raw: Json },
    Event { fields: JsonMap, raw: Json },
}

/// Discard, redact, map and keep one payload; `None` when the spec does not keep its event.
fn prepare(spec: &Spec, payload: Json, options: CaptureOptions, observer: &Observer<'_>) -> Option<NewRow> {
    let payload = spec.discarding(payload);
    let payload = if options.hide_secrets { redact(&payload) } else { payload };
    let fields = spec.extract(&payload);
    let event = fields.get("event").and_then(Json::as_str);
    if !spec.keeps(event) {
        return None;
    }
    if event.is_some_and(|e| spec.prompt_events.iter().any(|p| p == e)) {
        let context = observe(spec, fields.get("cwd").and_then(Json::as_str), observer);
        Some(NewRow::Prompt { fields, context, raw: payload })
    } else {
        Some(NewRow::Event { raw: clip(&payload, CLIP_LIMIT), fields })
    }
}

fn store_row(spec: &Spec, store: &Store, row: NewRow, received_at: Option<&str>, captured: &mut Captured) -> Result<()> {
    match row {
        NewRow::Prompt { fields, context, raw } => {
            let stored = match received_at {
                Some(at) => store.add_prompt_at(&spec.id, &fields, &context, &raw, at)?,
                None => store.add_prompt(&spec.id, &fields, &context, &raw)?,
            };
            let Some(prompt) = stored else { return Ok(()) }; // a duplicate prompt id
            if spec.join == Join::Session && !has_value(&fields, "prompt_id") {
                store.name_turn(prompt.id, &spec.id)?;
            }
            captured.prompts += 1;
            captured.rows.push(StoredRow::Prompt(prompt.id));
        }
        NewRow::Event { fields, raw } => {
            let fields = join_session(spec, store, fields)?;
            let event = match received_at {
                Some(at) => store.add_event_at(&spec.id, &fields, &raw, at)?,
                None => store.add_event(&spec.id, &fields, &raw)?,
            };
            captured.events += 1;
            captured.rows.push(StoredRow::Event(event.id));
            if let Some(turn) = text(&fields, "prompt_id") {
                captured.turns.insert(turn);
            }
        }
    }
    Ok(())
}

/// A mapped field holds a value (not null, `""` or `[]`).
fn has_value(fields: &JsonMap, key: &str) -> bool {
    fields.get(key).is_some_and(|value| !is_no_value(value))
}

/// A mapped string or number field as text; `None` when missing, empty or of another type.
fn text(fields: &JsonMap, key: &str) -> Option<String> {
    match fields.get(key)? {
        Json::String(s) if !s.is_empty() => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A `received_at` as the store writes it: UTC, microseconds, `+00:00`.
pub(crate) fn iso(at: Timestamp) -> String {
    at.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_received_at_is_written_like_the_store_writes_it() {
        let at: Timestamp = "2026-11-03T10:00:00.1234567Z".parse().unwrap();
        assert_eq!(iso(at), "2026-11-03T10:00:00.123456+00:00");
    }

    #[test]
    fn a_number_prompt_id_counts_as_text_and_an_empty_one_does_not() {
        let fields: JsonMap = serde_json::from_str(r#"{"a": 7, "b": "", "c": [], "d": "x"}"#).unwrap();
        assert_eq!((text(&fields, "a"), text(&fields, "b"), text(&fields, "d")), (Some("7".into()), None, Some("x".into())));
        assert_eq!((has_value(&fields, "b"), has_value(&fields, "c"), has_value(&fields, "a")), (false, false, true));
    }
}
