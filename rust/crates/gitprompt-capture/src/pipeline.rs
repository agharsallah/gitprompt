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
//! Synchronous on purpose: the store is sync, git and `--version` are blocking; the server runs
//! [`capture`] in `spawn_blocking`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use gitprompt_core::git::GitCache;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::{Format, Spec};
use gitprompt_core::{Json, JsonMap, Result};
use gitprompt_store::Store;

use crate::probe::VersionProbe;

/// Repository state of a working directory (`core::git::git_context`'s keys); `{}` outside a repo.
pub trait GitProbe: Send + Sync {
    fn context(&self, cwd: Option<&Path>) -> JsonMap;
}

/// `core::git::git_context`, uncached.
pub struct SystemGit;

impl GitProbe for SystemGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        todo!()
    }
}

impl GitProbe for GitCache {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        todo!()
    }
}

/// What the pipeline reads from this machine when a prompt arrives.
#[derive(Clone, Copy)]
pub struct Observer<'a> {
    pub git: &'a dyn GitProbe,
    pub versions: &'a dyn VersionProbe,
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
    pub fn for_source(_preferences: &Preferences, _source: &str) -> Self {
        todo!()
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
    /// prompt_ids that received events, so the caller can mark them for the prompt index.
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
        todo!()
    }

    pub fn notice(&self) -> Notice {
        todo!()
    }
}

/// The payloads of one request body: the body itself for `json` (one event per hook call), the
/// records of [`crate::otlp::records`] for `otlp`.
pub fn decode(_format: Format, _body: &Json) -> Vec<Json> {
    todo!()
}

/// What GitPrompt sees on this machine when a prompt arrives: the repository state of `cwd`, plus
/// `tool_version` when the tool reports one.
pub fn observe(_spec: &Spec, _cwd: Option<&str>, _observer: &Observer<'_>) -> JsonMap {
    todo!()
}

/// `join: session`: an event without a prompt id takes the id of the latest prompt of its session.
/// Unchanged for a `prompt_id` source, an event that has a prompt id or no session id, or a session
/// without a prompt yet.
pub fn join_session(_spec: &Spec, _store: &Store, _fields: JsonMap) -> Result<JsonMap> {
    todo!()
}

/// Run one request body through the pipeline (see the module docs).
pub fn capture(_spec: &Spec, _store: &Store, _body: &Json, _options: CaptureOptions, _observer: &Observer<'_>) -> Result<Captured> {
    todo!()
}
