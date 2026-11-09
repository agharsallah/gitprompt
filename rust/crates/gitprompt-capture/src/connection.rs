//! Connecting a source = merging its spec's `install.merge` into the tool's config file, with
//! consent: every change can be previewed (dry run) as the exact diff before it is written.
//! This module holds the workflow (port of `sources/connection.py` plus the capture-side parts of
//! `api/routes/sources.py`: enable/disable, remap); [`crate::installer`] holds the JSON-file
//! mechanics. It also computes a connector's state (README "Connectors", formerly computed in the
//! dashboard's `state.ts`).

use std::path::PathBuf;
use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::DateTime;
use serde::{Deserialize, Serialize};

use gitprompt_core::preferences::{PreferenceFile, Preferences};
use gitprompt_core::spec::Spec;
use gitprompt_core::{Json, Result, Settings};
use gitprompt_store::Store;
use gitprompt_store::store::SourceStats;

/// One config-file change, previewed or applied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub file: String,
    /// Unified diff; empty when there is nothing to change.
    pub diff: String,
    pub applied: bool,
    /// The backup taken before writing, when a file existed.
    pub backup: Option<String>,
}

/// GitPrompt's entries for one source in one tool config file.
pub struct Connection<'a> {
    pub spec: &'a Spec,
    pub settings: &'a Settings,
    /// Render with this token instead of the current one (token rotation preview).
    pub token: Option<String>,
    /// Time used to name backups; `None` = the local time when writing.
    pub now: Option<DateTime>,
}

impl<'a> Connection<'a> {
    pub fn new(spec: &'a Spec, settings: &'a Settings) -> Self {
        Connection { spec, settings, token: None, now: None }
    }

    /// The same connection rendered with `token` (e.g. [`crate::rotation::PLACEHOLDER`]).
    pub fn with_token(self, token: impl Into<String>) -> Self {
        Connection { token: Some(token.into()), ..self }
    }

    /// The same connection naming backups after `now`.
    pub fn at(self, now: DateTime) -> Self {
        Connection { now: Some(now), ..self }
    }

    /// `install.file`, expanded (`${VAR:-default}` from `settings.env`, `~` = `settings.user_home`).
    pub fn config_file(&self) -> PathBuf {
        todo!()
    }

    /// `<home>/installed/<id>.json`: the plain values added at connect time, so disconnect removes
    /// exactly those.
    pub fn record_file(&self) -> PathBuf {
        todo!()
    }

    /// `install.merge` with `{{receiver}}`, `{{source}}` (the spec id) and `{{token}}` (the override,
    /// else `settings.token()`) filled in.
    pub fn patch(&self) -> Result<Json> {
        todo!()
    }

    /// The config file exists and holds this source's receiver URL (`Spec::marker`).
    pub fn connected(&self) -> bool {
        todo!()
    }

    /// Connected, but the spec now adds more (or other) entries than what is installed. `false`
    /// when not connected or the file can't be parsed.
    pub fn outdated(&self) -> bool {
        todo!()
    }

    /// Remove our previous entries, then merge the patch: reconnecting never duplicates. Refused
    /// with `Error::PolicyBlocked` ("Blocked by settings GitPrompt won't override: <blocks>") when
    /// policy blocks it, before anything is read or written; `Error::Config` for a config file that
    /// can't be parsed (left untouched). A dry run writes nothing. Applying writes the file (backup
    /// first) and the record of the plain values added.
    pub fn connect(&self, _dry_run: bool) -> Result<Change> {
        todo!()
    }

    /// Remove our list entries and the recorded plain values the user hasn't changed since. Nothing
    /// to remove = an empty diff, not applied, no write.
    pub fn disconnect(&self, _dry_run: bool) -> Result<Change> {
        todo!()
    }
}

// --- enable, disable, remap (capture-side halves of the sources routes) ----------------------

/// `Error::Conflict` ("<name> is turned off in GitPrompt: enable it first") when the source is in
/// `disabled_sources`: a turned-off connector can't be connected.
pub fn refuse_if_disabled(_spec: &Spec, _preferences: &Preferences) -> Result<()> {
    todo!()
}

/// Turn a connector on or off in GitPrompt (preference `disabled_sources`); its entries stay in the
/// tool's config file. Other disabled sources are kept; an empty list removes the key. Returns the new
/// `enabled`. `Error::Locked` when `GITPROMPT_DISABLED_SOURCES` is set.
pub fn set_enabled(_spec: &Spec, _preferences: &PreferenceFile, _enabled: bool) -> Result<bool> {
    todo!()
}

/// Rebuild mapped fields from stored raw payloads (after a spec fix); for a `join: session` source
/// the prompt ids given at ingest are not in raw, so the session join is redone. Returns rows
/// remapped. (The server rebuilds the prompt index afterwards.)
pub fn remap(_spec: &Spec, _store: &Store) -> Result<u64> {
    todo!()
}

// --- connector state -------------------------------------------------------------------------

/// No event for longer than this = quiet. A day: gaps of hours are ordinary use.
pub const QUIET_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// The raw facts a connector's state is derived from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateFacts {
    pub enabled: bool,
    pub detected: bool,
    pub connected: bool,
    pub outdated: bool,
    pub blocks: Vec<String>,
    pub warnings: Vec<String>,
    /// Prompts stored without text.
    pub unmapped: u64,
    pub last_at: Option<Timestamp>,
}

impl StateFacts {
    /// Read the facts on this machine: preferences (enabled), [`crate::probe::detected`],
    /// [`Connection::connected`]/[`Connection::outdated`], [`crate::probe::check_policy`], and the
    /// store's per-source stats (`unmapped`, `last_at`; `None` = nothing stored).
    pub fn gather(_spec: &Spec, _settings: &Settings, _preferences: &Preferences, _stats: Option<&SourceStats>) -> Self {
        todo!()
    }
}

/// Every state a connector layer can be in; exactly one applies, decided in this order:
///
/// | either        | `Disabled`        | turned off in GitPrompt                             |
/// |---------------|-------------------|-----------------------------------------------------|
/// | not connected | `NotInstalled`    | the tool isn't on this machine                      |
/// |               | `BlockedByPolicy` | a setting GitPrompt won't override turns it off     |
/// |               | `Warning`         | can connect, but a setting may stop its entries     |
/// |               | `Available`       | ready to connect                                    |
/// | connected     | `BlockedByPolicy` | (the entries can't run)                             |
/// |               | `Outdated`        | the spec now installs more or other entries         |
/// |               | `Drift`           | prompts arrived without text                        |
/// |               | `Waiting`         | nothing received yet                                |
/// |               | `Quiet`           | nothing received for more than [`QUIET_AFTER`]     |
/// |               | `Receiving`       | events are arriving                                 |
///
/// A connected connector is never `NotInstalled` nor `Warning`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorState {
    Disabled,
    NotInstalled,
    Available,
    Warning,
    BlockedByPolicy,
    Outdated,
    Drift,
    Waiting,
    Quiet,
    Receiving,
}

/// The one primary action of a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Enable,
    Recheck,
    Connect,
    Update,
    Remap,
    View,
}

impl ConnectorState {
    /// Every state, in the order above.
    pub const ALL: [ConnectorState; 10] = [
        ConnectorState::Disabled,
        ConnectorState::NotInstalled,
        ConnectorState::Available,
        ConnectorState::Warning,
        ConnectorState::BlockedByPolicy,
        ConnectorState::Outdated,
        ConnectorState::Drift,
        ConnectorState::Waiting,
        ConnectorState::Quiet,
        ConnectorState::Receiving,
    ];

    pub fn of(_facts: &StateFacts, _now: Timestamp) -> Self {
        todo!()
    }

    /// The README label: "Turned off", "Not installed", "Ready to connect", "Check before
    /// connecting", "Blocked by policy", "Update available", "Payload drift", "Waiting for the first
    /// event", "Quiet", "Receiving".
    pub fn label(self) -> &'static str {
        todo!()
    }

    pub fn action(self) -> Action {
        todo!()
    }
}
