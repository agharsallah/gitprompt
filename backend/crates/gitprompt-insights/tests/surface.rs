//! Where a prompt was typed (surface.rs): the spec's surface rules, first match wins, unknown stays
//! None. Port of `tests/insights/test_surface.py` and the surface parts of `tests/api/test_desktop_apps.py`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::{custom_spec, event, prompt, spec, spec_yaml, specs, telemetry};
use gitprompt_core::spec::{Spec, SurfaceInfo};
use gitprompt_insights::surface::{catalog, signals, surface_of};
use gitprompt_store::EventRow;

/// Claude Code's surface for a prompt whose turn has a telemetry `user_prompt` with these attributes.
fn claude_surface(attrs: Value) -> Option<String> {
    surface_of(&prompt("hi").row(), &[telemetry(attrs)], Some(&spec("claude-code")))
}

fn pairs(signals: indexmap::IndexMap<String, Value>) -> Vec<(String, Value)> {
    signals.into_iter().collect()
}

#[test]
fn the_cli_entrypoint_is_the_cli() {
    assert_eq!(claude_surface(json!({"app.entrypoint": "cli", "service.name": "claude-code"})).as_deref(), Some("cli"));
}

#[test]
fn the_vscode_extension_entrypoint_is_vscode() {
    assert_eq!(claude_surface(json!({"app.entrypoint": "claude-vscode"})).as_deref(), Some("vscode"));
}

#[test]
fn every_sdk_entrypoint_is_the_sdk() {
    let found = ["sdk-cli", "sdk-ts", "sdk-py"].map(|e| claude_surface(json!({"app.entrypoint": e})));
    assert_eq!(found, [Some("sdk".to_string()), Some("sdk".to_string()), Some("sdk".to_string())]);
}

#[test]
fn both_desktop_entrypoints_are_the_desktop_app() {
    let found = ["claude-desktop", "claude-desktop-3p"].map(|e| claude_surface(json!({"app.entrypoint": e})));
    assert_eq!(found, [Some("desktop".to_string()), Some("desktop".to_string())]);
}

#[test]
fn the_slack_entrypoint_is_slack() {
    assert_eq!(claude_surface(json!({"app.entrypoint": "claude-in-slack"})).as_deref(), Some("slack"));
}

#[test]
fn the_documented_desktop_service_name_wins_over_the_entrypoint() {
    assert_eq!(claude_surface(json!({"service.name": "claude-code-desktop", "app.entrypoint": "sdk-ts"})).as_deref(), Some("desktop"));
}

#[test]
fn an_unknown_entrypoint_is_never_guessed() {
    assert_eq!(claude_surface(json!({"app.entrypoint": "something-new"})), None);
}

#[test]
fn telemetry_without_attributes_gives_no_surface() {
    assert_eq!(claude_surface(json!({})), None);
}

#[test]
fn only_telemetry_user_prompt_events_carry_the_entrypoint() {
    let api_request = event("api_request", json!({"app.entrypoint": "cli"}));
    assert_eq!(surface_of(&prompt("hi").row(), &[api_request], Some(&spec("claude-code"))), None);
}

#[test]
fn without_telemetry_the_surface_is_unknown() {
    let row = prompt("hi").raw("hook_event_name", json!("UserPromptSubmit")).row();
    assert_eq!(surface_of(&row, &[], Some(&spec("claude-code"))), None);
}

#[test]
fn a_payload_rule_needs_no_event() {
    let row = prompt("hi").raw("cursor_version", json!("3.4.1")).row();
    assert_eq!(surface_of(&row, &[], Some(&spec("claude-code"))).as_deref(), Some("cursor"));
}

#[test]
fn tools_without_surface_rules_never_guess() {
    let row = prompt("hi").raw("cwd", json!("/w")).raw("model", json!("m")).row();
    let events = [telemetry(json!({"app.entrypoint": "cli"}))];
    let found = ["codex", "copilot-cli", "cursor"].map(|tool| surface_of(&row, &events, Some(&spec(tool))));
    assert_eq!(found, [None, None, None]);
}

#[test]
fn without_a_spec_there_is_no_surface() {
    assert_eq!(surface_of(&prompt("hi").row(), &[telemetry(json!({"app.entrypoint": "cli"}))], None), None);
}

#[test]
fn signals_show_what_the_rules_read_in_rule_order() {
    let events = [telemetry(json!({"app.entrypoint": "weird", "service.name": "claude-code"}))];
    assert_eq!(
        pairs(signals(&prompt("hi").row(), &events, Some(&spec("claude-code")))),
        vec![("service.name".to_string(), json!("claude-code")), ("app.entrypoint".to_string(), json!("weird"))]
    );
}

#[test]
fn signals_read_payload_paths_from_the_prompt_itself() {
    let row = prompt("hi").raw("cursor_version", json!("3.4.1")).row();
    assert_eq!(pairs(signals(&row, &[], Some(&spec("claude-code")))), vec![("cursor_version".to_string(), json!("3.4.1"))]);
}

#[test]
fn signals_take_the_first_event_that_has_a_value() {
    let events: Vec<EventRow> =
        vec![telemetry(json!({})), telemetry(json!({"app.entrypoint": "cli"})), telemetry(json!({"app.entrypoint": "sdk-ts"}))];
    assert_eq!(
        pairs(signals(&prompt("hi").row(), &events, Some(&spec("claude-code")))),
        vec![("app.entrypoint".to_string(), json!("cli"))]
    );
}

#[test]
fn signals_are_empty_without_values_or_a_spec() {
    assert!(signals(&prompt("hi").row(), &[], Some(&spec("claude-code"))).is_empty());
    assert!(signals(&prompt("hi").row(), &[telemetry(json!({"app.entrypoint": "cli"}))], None).is_empty());
}

#[test]
fn rules_and_surface_names_are_spec_data() {
    let kiosk = custom_spec(&spec_yaml(
        "surfaces: {kiosk: {name: Kiosk, icon: app}}\nsurface: [{when: {path: client, equals: k}, surface: kiosk}]\n",
    ));
    assert_eq!(surface_of(&prompt("hi").raw("client", json!("k")).row(), &[], Some(&kiosk)).as_deref(), Some("kiosk"));
    assert_eq!(surface_of(&prompt("hi").raw("client", json!("j")).row(), &[], Some(&kiosk)), None);
}

#[test]
fn the_catalog_names_the_shared_surfaces_and_each_spec_own() {
    let kiosk = custom_spec(&spec_yaml("surfaces: {kiosk: {name: Kiosk, icon: app}}\n"));
    let found = catalog([&kiosk]);
    assert_eq!(found.get("kiosk"), Some(&SurfaceInfo { name: "Kiosk".into(), icon: "app".into() }));
    assert_eq!(found.get("cli"), Some(&SurfaceInfo { name: "CLI".into(), icon: "terminal".into() }));
}

#[test]
fn a_later_spec_renames_a_surface_in_the_catalog() {
    let first = custom_spec(&spec_yaml("surfaces: {kiosk: {name: Kiosk, icon: app}}\n"));
    let second = custom_spec(&spec_yaml("surfaces: {kiosk: {name: Lobby kiosk, icon: app}}\n"));
    assert_eq!(catalog([&first, &second])["kiosk"].name, "Lobby kiosk");
}

#[test]
fn the_catalog_of_no_spec_is_empty() {
    assert!(catalog(std::iter::empty::<&Spec>()).is_empty());
}

#[test]
fn every_surface_a_bundled_spec_captures_or_derives_is_in_the_catalog() {
    let all = specs();
    let named = catalog(all.values().map(std::convert::AsRef::as_ref));
    let used: Vec<String> =
        all.values().flat_map(|s| s.surface.iter().map(|r| r.surface.clone()).chain(s.captures.keys().cloned())).collect();
    let unnamed: Vec<&String> = used.iter().filter(|s| !named.contains_key(*s)).collect();
    assert_eq!(unnamed, Vec::<&String>::new());
}
