//! A connector layer's state (README "Connectors"): derived from raw facts read on this machine and
//! in the store. Formerly computed in the dashboard's `state.ts`.

use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

use gitprompt_core::Settings;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::Spec;
use gitprompt_store::store::SourceStats;

use super::Connection;
use crate::probe::{check_policy, detected};

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
    pub fn gather(spec: &Spec, settings: &Settings, preferences: &Preferences, stats: Option<&SourceStats>) -> Self {
        Self::gather_for(&Connection::new(spec, settings), preferences, stats)
    }

    /// [`Self::gather`] for a connection as the caller installs it (e.g. with a forwarder, so
    /// `outdated` compares against the same form of hook command).
    pub fn gather_for(connection: &Connection<'_>, preferences: &Preferences, stats: Option<&SourceStats>) -> Self {
        let (spec, settings) = (connection.spec, connection.settings);
        let policy = check_policy(spec, settings);
        StateFacts {
            enabled: !preferences.disabled_sources.contains(&spec.id),
            detected: detected(spec, settings),
            connected: connection.connected(),
            outdated: connection.outdated(),
            blocks: policy.blocks,
            warnings: policy.warnings,
            unmapped: stats.map_or(0, |s| s.unmapped),
            last_at: stats.and_then(|s| s.last_at.as_deref()).and_then(|at| parse_time(&spec.id, at)),
        }
    }
}

/// A stored `received_at`; `None` (logged) when it isn't a time.
fn parse_time(source: &str, text: &str) -> Option<Timestamp> {
    text.parse().map_err(|e| tracing::warn!(%source, received_at = text, error = %e, "unreadable last activity time")).ok()
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

    pub fn of(facts: &StateFacts, now: Timestamp) -> Self {
        use ConnectorState::{Available, BlockedByPolicy, Disabled, Drift, NotInstalled, Outdated, Quiet, Receiving, Waiting, Warning};
        if !facts.enabled {
            return Disabled;
        }
        if !facts.connected {
            return if !facts.detected {
                NotInstalled
            } else if !facts.blocks.is_empty() {
                BlockedByPolicy
            } else if !facts.warnings.is_empty() {
                Warning
            } else {
                Available
            };
        }
        if !facts.blocks.is_empty() {
            return BlockedByPolicy;
        }
        if facts.outdated {
            return Outdated;
        }
        if facts.unmapped > 0 {
            return Drift;
        }
        match facts.last_at {
            None => Waiting,
            Some(last) if now.duration_since(last) > quiet_after() => Quiet,
            Some(_) => Receiving,
        }
    }

    /// The README label: "Turned off", "Not installed", "Ready to connect", "Check before
    /// connecting", "Blocked by policy", "Update available", "Payload drift", "Waiting for the first
    /// event", "Quiet", "Receiving".
    pub fn label(self) -> &'static str {
        match self {
            ConnectorState::Disabled => "Turned off",
            ConnectorState::NotInstalled => "Not installed",
            ConnectorState::Available => "Ready to connect",
            ConnectorState::Warning => "Check before connecting",
            ConnectorState::BlockedByPolicy => "Blocked by policy",
            ConnectorState::Outdated => "Update available",
            ConnectorState::Drift => "Payload drift",
            ConnectorState::Waiting => "Waiting for the first event",
            ConnectorState::Quiet => "Quiet",
            ConnectorState::Receiving => "Receiving",
        }
    }

    pub fn action(self) -> Action {
        match self {
            ConnectorState::Disabled => Action::Enable,
            ConnectorState::NotInstalled | ConnectorState::BlockedByPolicy | ConnectorState::Waiting => Action::Recheck,
            ConnectorState::Available | ConnectorState::Warning => Action::Connect,
            ConnectorState::Outdated => Action::Update,
            ConnectorState::Drift => Action::Remap,
            ConnectorState::Quiet | ConnectorState::Receiving => Action::View,
        }
    }
}

/// [`QUIET_AFTER`] as a signed duration (a day always fits).
fn quiet_after() -> SignedDuration {
    SignedDuration::try_from(QUIET_AFTER).unwrap_or(SignedDuration::MAX)
}
