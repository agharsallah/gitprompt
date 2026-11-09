//! A connector layer's state (README "Connectors"), from its raw facts, and gathering those facts.

mod common;

use jiff::civil::date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use common::*;
use gitprompt_capture::connection::{Action, Connection, ConnectorState, QUIET_AFTER, StateFacts};
use gitprompt_core::preferences::Preferences;
use gitprompt_store::store::SourceStats;
use pretty_assertions::assert_eq;
use serde_json::json;

use ConnectorState::*;

fn now() -> Timestamp {
    date(2026, 11, 7).at(15, 40, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp()
}

/// Installed, not connected, nothing in the way.
fn facts() -> StateFacts {
    StateFacts {
        enabled: true,
        detected: true,
        connected: false,
        outdated: false,
        blocks: vec![],
        warnings: vec![],
        unmapped: 0,
        last_at: None,
    }
}

fn connected() -> StateFacts {
    StateFacts { connected: true, last_at: Some(now()), ..facts() }
}

fn state(f: StateFacts) -> ConnectorState {
    ConnectorState::of(&f, now())
}

#[test]
fn a_turned_off_connector_is_disabled_before_anything_else() {
    assert_eq!(state(StateFacts { enabled: false, detected: false, ..facts() }), Disabled);
    assert_eq!(state(StateFacts { enabled: false, blocks: vec!["b".into()], ..connected() }), Disabled);
}

#[test]
fn a_tool_missing_from_this_machine_is_not_installed_even_when_policy_blocks() {
    assert_eq!(state(StateFacts { detected: false, blocks: vec!["b".into()], ..facts() }), NotInstalled);
}

#[test]
fn an_installed_tool_is_ready_to_connect_or_needs_a_check_when_a_setting_may_interfere() {
    assert_eq!(state(facts()), Available);
    assert_eq!(state(StateFacts { warnings: vec!["w".into()], ..facts() }), Warning);
}

#[test]
fn policy_blocks_outrank_warnings_and_updates() {
    let blocks = vec!["disableAllHooks is set in managed-settings.json".to_string()];
    assert_eq!(state(StateFacts { blocks: blocks.clone(), warnings: vec!["w".into()], ..facts() }), BlockedByPolicy);
    assert_eq!(state(StateFacts { blocks, outdated: true, ..connected() }), BlockedByPolicy);
}

#[test]
fn an_update_outranks_drift_and_drift_outranks_activity() {
    assert_eq!(state(StateFacts { outdated: true, unmapped: 2, ..connected() }), Outdated);
    assert_eq!(state(StateFacts { unmapped: 2, ..connected() }), Drift);
}

#[test]
fn a_connected_connector_is_never_not_installed_nor_a_warning() {
    let waiting = StateFacts { last_at: None, ..connected() };
    assert_eq!(state(StateFacts { detected: false, ..waiting.clone() }), Waiting);
    assert_eq!(state(StateFacts { warnings: vec!["w".into()], ..waiting }), Waiting);
}

#[test]
fn a_connector_turns_quiet_only_after_a_full_day_without_events() {
    let day = SignedDuration::try_from(QUIET_AFTER).unwrap();
    let at = |ago: SignedDuration| state(StateFacts { last_at: Some(now() - ago), ..connected() });
    assert_eq!(at(day), Receiving);
    assert_eq!(at(day + SignedDuration::from_millis(1)), Quiet);
}

#[test]
fn every_state_has_a_distinct_label() {
    let labels: Vec<&str> = ConnectorState::ALL.iter().map(|s| s.label()).collect();
    assert_eq!(
        labels,
        vec![
            "Turned off",
            "Not installed",
            "Ready to connect",
            "Check before connecting",
            "Blocked by policy",
            "Update available",
            "Payload drift",
            "Waiting for the first event",
            "Quiet",
            "Receiving"
        ]
    );
}

#[test]
fn every_state_has_the_one_action_the_readme_names() {
    let actions: Vec<Action> = ConnectorState::ALL.iter().map(|s| s.action()).collect();
    use Action::*;
    assert_eq!(actions, vec![Enable, Recheck, Connect, Connect, Recheck, Update, Remap, Recheck, View, View]);
}

#[test]
fn states_serialize_as_the_dashboards_ids() {
    let ids = serde_json::to_value(ConnectorState::ALL).unwrap();
    assert_eq!(
        ids,
        json!([
            "disabled",
            "not_installed",
            "available",
            "warning",
            "blocked_by_policy",
            "outdated",
            "drift",
            "waiting",
            "quiet",
            "receiving"
        ])
    );
}

// --- gathering the facts ---------------------------------------------------------------------

#[test]
fn facts_of_a_connected_receiving_connector_come_from_this_machine_and_the_store() {
    let env = TestEnv::new();
    write_json(&env.claude_settings(), &json!({}));
    let spec = env.spec("claude-code");
    Connection::new(&spec, &env.settings).connect(false).unwrap();
    let stats = SourceStats { total: 3, unmapped: 1, events: 5, last_at: Some("2026-11-07T15:00:00.000000+00:00".into()) };

    let facts = StateFacts::gather(&spec, &env.settings, &Preferences::default(), Some(&stats));

    assert_eq!(
        facts,
        StateFacts {
            enabled: true,
            detected: true,
            connected: true,
            outdated: false,
            blocks: vec![],
            warnings: vec![],
            unmapped: 1,
            last_at: Some(date(2026, 11, 7).at(15, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp()),
        }
    );
}

#[test]
fn facts_of_a_turned_off_tool_that_is_not_installed_and_never_sent_anything() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let prefs = Preferences { disabled_sources: vec!["cursor".into()], ..Preferences::default() };
    let facts = StateFacts::gather(&spec, &env.settings, &prefs, None);
    assert_eq!((facts.enabled, facts.detected, facts.connected, facts.unmapped, facts.last_at), (false, false, false, 0, None));
}

#[test]
fn gathered_facts_carry_policy_blocks_and_warnings() {
    let env = TestEnv::with_env(&[("CLAUDE_CODE_ENABLE_TELEMETRY", "1")]);
    write_json(&env.claude_settings(), &json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://collector.example.com"}}));
    let facts = StateFacts::gather(&env.spec("claude-code-otel"), &env.settings, &Preferences::default(), None);
    assert_eq!((facts.blocks.len(), facts.warnings.len()), (1, 1));
    assert_eq!(ConnectorState::of(&facts, now()), BlockedByPolicy);
}
