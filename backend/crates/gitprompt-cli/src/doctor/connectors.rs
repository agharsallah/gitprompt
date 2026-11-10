//! Per connector: detected, connected, how its hooks are installed, its state as the Connectors
//! page computes it, what it delivered. Read-only: no tool version is run, nothing is written.

use std::collections::BTreeMap;
use std::path::Path;

use jiff::Timestamp;

use gitprompt_capture::connection::{Connection, ConnectorState, HookForm, StateFacts};
use gitprompt_capture::rotation::PLACEHOLDER;
use gitprompt_core::Settings;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::SpecSet;
use gitprompt_store::store::SourceStats;

use super::report::ConnectorCheck;

/// Every source of `set`, in id order. `token`: the current receiver token (`None` when there is
/// none yet; the comparison then uses a placeholder, so the probe never creates one).
pub(super) fn check(
    set: &SpecSet,
    settings: &Settings,
    preferences: &Preferences,
    stats: &BTreeMap<String, SourceStats>,
    token: Option<&str>,
    now: Timestamp,
) -> Vec<ConnectorCheck> {
    set.specs
        .values()
        .map(|spec| {
            let plain = Connection::new(spec, settings);
            let install = plain.installed_form();
            let forwarder = match &install {
                Some(HookForm::Forwarder { path: Some(path) }) => Some(Path::new(path)),
                _ => None,
            };
            // Compare against the form that is installed, as the app that installed it would.
            let connection = Connection::new(spec, settings).via(forwarder).with_token(token.unwrap_or(PLACEHOLDER));
            let source_stats = stats.get(&spec.id);
            let facts = StateFacts::gather_for(&connection, preferences, source_stats);
            let state = ConnectorState::of(&facts, now);
            ConnectorCheck {
                id: spec.id.clone(),
                name: spec.name.clone(),
                part_of: spec.part_of.clone(),
                enabled: facts.enabled,
                detected: facts.detected,
                connected: facts.connected,
                install,
                config_file: connection.config_file().display().to_string(),
                state,
                state_label: state.label().to_owned(),
                prompts: source_stats.map_or(0, |s| s.total),
                events: source_stats.map_or(0, |s| s.events),
                last_at: source_stats.and_then(|s| s.last_at.clone()),
            }
        })
        .collect()
}
