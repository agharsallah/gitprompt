//! /api/sources: the raw facts per connector, connect/disconnect with consent (dry run first),
//! policy blocks, enable/disable, remap, the spec in use and broken spec files. Port of
//! test_api.py, test_sources.py, test_connectors.py (management parts), and the broken-spec test
//! of test_extension_points.py.

mod common;

use std::os::unix::fs::PermissionsExt;

use common::{Api, api, claude_settings, harness, hook_payload, read_json, with};
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const HOURS: usize = 24;

fn dashboard_post<'a>(api: &'a Api, path: &str) -> common::Req<'a> {
    api.post(path).dashboard()
}

// --- connect / disconnect ---

fn users_claude_settings() -> Value {
    json!({"theme": "dark", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}})
}

fn write_settings(path: &std::path::Path, value: &Value) {
    std::fs::write(path, serde_json::to_string(value).expect("json")).expect("write");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
}

#[tokio::test]
async fn an_installed_tool_is_detected_and_not_yet_connected() {
    let api = api();
    write_settings(&claude_settings(&api.settings), &users_claude_settings());
    let found = api.source("claude-code").await;
    assert_eq!((&found["detected"], &found["connected"]), (&json!(true), &json!(false)));
}

#[tokio::test]
async fn a_connect_dry_run_shows_the_diff_and_changes_nothing() {
    let api = api();
    let path = claude_settings(&api.settings);
    write_settings(&path, &users_claude_settings());
    let preview = dashboard_post(&api, "/api/sources/claude-code/connect?dry_run=true").send().await.json();
    assert!(preview["diff"].as_str().expect("diff").contains("/ingest/claude-code"), "{preview}");
    assert_eq!(preview["applied"], false);
    assert_eq!(read_json(&path), users_claude_settings());
}

#[tokio::test]
async fn connect_merges_into_the_users_settings_keeping_their_mode_and_backs_them_up() {
    let api = api();
    let path = claude_settings(&api.settings);
    write_settings(&path, &users_claude_settings());
    let applied = dashboard_post(&api, "/api/sources/claude-code/connect").send().await.json();
    assert_eq!(applied["applied"], true);
    assert!(applied["backup"].as_str().is_some_and(|b| !b.is_empty()), "{applied}");
    let written = read_json(&path);
    assert_eq!(written["theme"], "dark");
    assert!(written["hooks"].get("Stop").is_some() && written["hooks"].get("UserPromptSubmit").is_some(), "{written}");
    assert_eq!(std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777, 0o600);
    assert_eq!(
        (api.source("claude-code").await["connected"].clone(), api.source("claude-code-otel").await["connected"].clone()),
        (json!(true), json!(false))
    );
}

#[tokio::test]
async fn disconnect_restores_the_users_settings_exactly() {
    let api = api();
    let path = claude_settings(&api.settings);
    write_settings(&path, &users_claude_settings());
    dashboard_post(&api, "/api/sources/claude-code/connect").send().await;
    let removed = dashboard_post(&api, "/api/sources/claude-code/disconnect").send().await.json();
    assert_eq!(removed["applied"], true);
    assert_eq!(read_json(&path), users_claude_settings());
}

#[tokio::test]
async fn a_policy_that_disables_all_hooks_blocks_connecting_and_leaves_the_file_alone() {
    let api = api();
    let path = claude_settings(&api.settings);
    write_settings(&path, &json!({"disableAllHooks": true}));
    let response = dashboard_post(&api, "/api/sources/claude-code/connect").send().await;
    assert_eq!(response.status, 409);
    assert!(response.text().contains("disableAllHooks"), "{}", response.text());
    assert_eq!(read_json(&path), json!({"disableAllHooks": true}));
}

#[tokio::test]
async fn a_strict_plugin_only_policy_blocks_connecting() {
    let api = api();
    write_settings(&claude_settings(&api.settings), &json!({"strictPluginOnlyCustomization": ["skills", "hooks"]}));
    assert_eq!(dashboard_post(&api, "/api/sources/claude-code/connect").send().await.status, 409);
}

#[tokio::test]
async fn invalid_tool_settings_are_never_rewritten() {
    let api = api();
    let path = claude_settings(&api.settings);
    std::fs::write(&path, "{oops").expect("write");
    assert_eq!(dashboard_post(&api, "/api/sources/claude-code/connect").send().await.status, 409);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{oops");
}

#[tokio::test]
async fn telemetry_connect_keeps_the_users_env_and_each_disconnect_removes_only_its_own_entries() {
    let api = api();
    let path = claude_settings(&api.settings);
    let original = json!({"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1", "MY_VAR": "x"}});
    write_settings(&path, &original);
    dashboard_post(&api, "/api/sources/claude-code/connect").send().await;
    dashboard_post(&api, "/api/sources/claude-code-otel/connect").send().await;
    let written = read_json(&path);
    assert!(written["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"].as_str().expect("endpoint").ends_with("/ingest/claude-code-otel"));
    assert!(written["hooks"].get("Stop").is_some());
    let telemetry = api.source("claude-code-otel").await;
    assert_eq!((&telemetry["connected"], &telemetry["blocks"]), (&json!(true), &json!([])));
    assert_eq!(dashboard_post(&api, "/api/sources/claude-code-otel/connect?dry_run=true").send().await.json()["diff"], "");

    dashboard_post(&api, "/api/sources/claude-code-otel/disconnect").send().await;
    assert_eq!(read_json(&path), with(&original, json!({"hooks": written["hooks"].clone()}))); // the hooks source untouched
    dashboard_post(&api, "/api/sources/claude-code/disconnect").send().await;
    assert_eq!(read_json(&path), original);
}

#[tokio::test]
async fn telemetry_already_sent_elsewhere_is_not_redirected() {
    let api = api();
    write_settings(&claude_settings(&api.settings), &json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://collector.example.com"}}));
    let response = dashboard_post(&api, "/api/sources/claude-code-otel/connect").send().await;
    assert_eq!(response.status, 409);
    assert!(response.text().contains("OTEL_EXPORTER_OTLP_ENDPOINT"), "{}", response.text());
}

#[tokio::test]
async fn an_older_connection_is_reported_outdated_and_updated_in_place() {
    let api = api();
    let path = claude_settings(&api.settings);
    let old = json!({"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:7878/ingest/claude-code"}]}]}});
    write_settings(&path, &old);
    let found = api.source("claude-code").await;
    assert_eq!((&found["connected"], &found["outdated"]), (&json!(true), &json!(true)));
    dashboard_post(&api, "/api/sources/claude-code/connect").send().await;
    let written = read_json(&path);
    assert_eq!(written["hooks"]["UserPromptSubmit"].as_array().map(Vec::len), Some(1)); // replaced, not duplicated
    assert!(written["hooks"].get("Stop").is_some());
    assert_eq!(api.source("claude-code").await["outdated"], false);
}

#[tokio::test]
async fn connecting_an_unknown_source_is_404() {
    let api = api();
    assert_eq!(dashboard_post(&api, "/api/sources/nope/connect").send().await.status, 404);
}

// --- copilot: a hooks file of its own ---

fn copilot_hooks_dir(api: &Api) -> std::path::PathBuf {
    let dir = api.settings.user_home.join(".copilot");
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir.join("hooks")
}

#[tokio::test]
async fn copilot_connect_writes_a_hooks_file_of_its_own_and_disconnect_empties_it() {
    let api = api();
    let hooks = copilot_hooks_dir(&api);
    assert_eq!(api.source("copilot-cli").await["detected"], true);
    let applied = dashboard_post(&api, "/api/sources/copilot-cli/connect").send().await.json();
    let written = read_json(&hooks.join("gitprompt.json"));
    assert_eq!((&applied["applied"], &written["version"]), (&json!(true), &json!(1)));
    let handlers = written["hooks"]["UserPromptSubmit"].as_array().expect("handlers");
    assert_eq!(handlers.len(), 1);
    let bash = handlers[0]["bash"].as_str().expect("bash");
    assert_eq!(handlers[0]["type"], "command");
    assert!(bash.contains("/ingest/copilot-cli") && !bash.contains("{{"), "{bash}");
    assert_eq!(
        (api.source("copilot-cli").await["connected"].clone(), api.source("claude-code").await["connected"].clone()),
        (json!(true), json!(false))
    );

    dashboard_post(&api, "/api/sources/copilot-cli/disconnect").send().await;
    assert_eq!(read_json(&hooks.join("gitprompt.json")), json!({}));
    assert_eq!(api.source("copilot-cli").await["connected"], false);
}

#[tokio::test]
async fn copilot_settings_that_disable_hooks_block_connecting() {
    let api = api();
    let hooks = copilot_hooks_dir(&api);
    std::fs::write(hooks.parent().expect("dir").join("settings.json"), r#"{"disableAllHooks": true}"#).expect("write");
    assert_ne!(api.source("copilot-cli").await["blocks"], json!([]));
    assert_eq!(dashboard_post(&api, "/api/sources/copilot-cli/connect").send().await.status, 409);
}

// --- enable / disable ---

fn copilot_prompt() -> Value {
    json!({"hook_event_name": "UserPromptSubmit", "session_id": "8f14e45f", "timestamp": "2026-11-07T10:00:00.000Z", "cwd": "/work/app", "prompt": "hi"})
}

#[tokio::test]
async fn a_disabled_connector_discards_payloads_and_refuses_to_connect() {
    let api = api();
    copilot_hooks_dir(&api);
    assert_eq!(dashboard_post(&api, "/api/sources/copilot-cli/disable").send().await.json(), json!({"enabled": false}));
    assert_eq!(
        (api.source("copilot-cli").await["enabled"].clone(), api.source("claude-code").await["enabled"].clone()),
        (json!(false), json!(true))
    );

    api.ingest("copilot-cli", copilot_prompt()).await;
    assert_eq!(api.source("copilot-cli").await["total"], 0);
    api.ingest("claude-code", hook_payload(&api.root())).await;
    assert_eq!(api.source("claude-code").await["total"], 1); // other tools are untouched
    let refused = dashboard_post(&api, "/api/sources/copilot-cli/connect?dry_run=true").send().await;
    assert_eq!(refused.status, 409);
}

#[tokio::test]
async fn enabling_again_resumes_capture_and_clears_the_preference() {
    let api = api();
    dashboard_post(&api, "/api/sources/copilot-cli/disable").send().await;
    assert_eq!(dashboard_post(&api, "/api/sources/copilot-cli/enable").send().await.json(), json!({"enabled": true}));
    api.ingest("copilot-cli", copilot_prompt()).await;
    assert_eq!(api.source("copilot-cli").await["total"], 1);
    assert_eq!(api.json("/api/settings").await["settings"]["disabled_sources"]["value"], json!([]));
}

#[tokio::test]
async fn connectors_disabled_by_env_are_reported_and_locked() {
    let api = harness().env("GITPROMPT_DISABLED_SOURCES", "copilot-cli, claude-code-otel").start();
    assert_eq!(
        (api.source("copilot-cli").await["enabled"].clone(), api.source("claude-code-otel").await["enabled"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(dashboard_post(&api, "/api/sources/copilot-cli/enable").send().await.status, 409);
}

// --- remap ---

#[tokio::test]
async fn remap_applies_an_edited_user_spec_to_stored_payloads() {
    let api = api();
    let payload = with(&hook_payload(&api.root()), json!({"prompt": null, "text": "typed"}));
    api.ingest("claude-code", payload).await;
    assert_eq!(api.source("claude-code").await["unmapped"], 0); // "text" is a default candidate
    std::fs::create_dir_all(api.settings.user_sources()).expect("mkdir");
    let spec = "id: claude-code\nname: Claude Code\ndetect: []\ninstall: {file: x, merge: {}}\nmap: {prompt: [nope]}\n";
    std::fs::write(api.settings.user_sources().join("claude-code.yaml"), spec).expect("write");
    assert_eq!(dashboard_post(&api, "/api/sources/claude-code/remap").send().await.json(), json!({"updated": 1}));
    assert_eq!(api.source("claude-code").await["unmapped"], 1);
}

// --- what each connector reports ---

#[tokio::test]
async fn sources_report_layers_activity_and_what_they_collect() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let hooks = api.source("claude-code").await;
    let telemetry = api.source("claude-code-otel").await;

    assert_eq!((&hooks["part_of"], &hooks["layer"]), (&Value::Null, &json!("Hooks")));
    assert!(hooks["summary"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!((&telemetry["part_of"], &telemetry["layer"]), (&json!("claude-code"), &json!("Telemetry")));
    let hourly = hooks["hourly"].as_array().expect("hourly");
    assert_eq!((hourly.len(), hourly.last().cloned()), (HOURS, Some(json!(1))));
    assert_eq!(telemetry["hourly"], json!(vec![0; HOURS]));

    let collects = hooks["collects"].as_array().expect("collects");
    assert!(collects.contains(&json!("UserPromptSubmit")) && !collects.contains(&json!("*")), "{collects:?}");
    assert!(hooks["not_collected"].get("PreToolUse").is_some());
    assert!(telemetry["not_stored"].get("hook_registered").is_some());
    let telemetry_collects = telemetry["collects"].as_array().expect("collects");
    assert!(telemetry_collects.contains(&json!("api_request")) && !telemetry_collects.contains(&json!("hook_registered")));
}

#[tokio::test]
async fn sources_report_the_installed_version_and_counts() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    let hooks = api.source("claude-code").await;
    assert_eq!(
        (&hooks["version"], &hooks["total"], &hooks["unmapped"], &hooks["events"]),
        (&json!("2.1.316"), &json!(1), &json!(0), &json!(1))
    );
    assert!(hooks["last_at"].is_string());
}

fn at(time: Timestamp) -> String {
    time.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
}

#[tokio::test]
async fn hourly_counts_prompts_and_events_per_hour_with_this_hour_last() {
    let now: Timestamp = "2026-11-07T15:40:00Z".parse().expect("time");
    let api = harness().clock(now).start();
    let stop = with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}));
    let event_times = [
        now - SignedDuration::from_hours(24), // 15:40 yesterday: just outside the window
        now - SignedDuration::from_hours(23) - SignedDuration::from_mins(30), // 16:10 yesterday: the first slot
        now - SignedDuration::from_hours(3),  // three slots back
        now - SignedDuration::from_mins(30),  // this hour
    ];
    for _ in event_times {
        api.ingest("claude-code", stop.clone()).await;
    }
    api.ingest("claude-code", hook_payload(&api.root())).await;
    for (id, time) in (1..).zip(event_times) {
        api.sql(&format!("update event set received_at = '{}' where id = {id}", at(time)));
    }
    api.sql(&format!("update prompt set received_at = '{}' where id = 1", at(now - SignedDuration::from_mins(5))));

    let mut expected = vec![0; HOURS];
    expected[0] = 1;
    expected[HOURS - 4] = 1;
    expected[HOURS - 1] = 2;
    assert_eq!(api.source("claude-code").await["hourly"], json!(expected));
}

#[tokio::test]
async fn hourly_leaves_out_rows_older_than_the_window() {
    let now: Timestamp = "2026-11-07T15:40:00Z".parse().expect("time");
    let api = harness().clock(now).start();
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    api.ingest_otlp("claude-code-otel", common::otlp(json!({}), json!({"event.name": "api_request", "prompt.id": "p1"}))).await;
    api.sql(&format!("update event set received_at = '{}' where id = 1", at(now - SignedDuration::from_hours(24 * 3))));
    api.sql(&format!("update event set received_at = '{}' where id = 2", at(now - SignedDuration::from_mins(10))));

    let mut telemetry = vec![0; HOURS];
    telemetry[HOURS - 1] = 1;
    assert_eq!(api.source("claude-code").await["hourly"], json!(vec![0; HOURS]));
    assert_eq!(api.source("claude-code-otel").await["hourly"], json!(telemetry));
}

#[tokio::test]
async fn sources_report_capabilities_join_and_the_spec_file_in_use() {
    let api = api();
    let hooks = api.source("claude-code").await;
    let telemetry = api.source("claude-code-otel").await;
    let copilot = api.source("copilot-cli").await;
    let has = |s: &Value, c: &str| s["capabilities"].as_array().expect("capabilities").contains(&json!(c));
    assert!(has(&telemetry, "usage") && !has(&hooks, "usage"));
    assert!(!has(&copilot, "model") && has(&copilot, "turn_end"));
    assert_eq!((&copilot["join"], &hooks["join"]), (&json!("session"), &json!("prompt_id")));
    assert_eq!((&copilot["spec"]["origin"], &copilot["spec"]["name"]), (&json!("bundled"), &json!("copilot-cli.yaml")));
    assert_eq!(copilot["spec_problem"], Value::Null);
}

#[tokio::test]
async fn the_spec_route_returns_the_yaml_in_use() {
    let api = api();
    let text = api.json("/api/sources/copilot-cli/spec").await;
    assert!(text["file"]["path"].as_str().expect("path").ends_with("copilot-cli.yaml"));
    assert!(text["text"].as_str().expect("text").contains("id: copilot-cli"));
    assert_eq!(api.get("/api/sources/nope/spec").send().await.status, 404);
}

#[tokio::test]
async fn sources_list_the_apps_each_connector_captures_and_cannot() {
    let api = api();
    let claude = api.source("claude-code").await;
    let ids = |s: &Value| {
        s["captures"]
            .as_array()
            .expect("captures")
            .iter()
            .map(|c| c["id"].as_str().expect("id").to_string())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(ids(&claude), ["cli", "desktop", "sdk", "vscode"].map(String::from).into());
    assert!(claude["cannot_capture"].get("Claude Desktop chat").is_some());
    assert!(ids(&api.source("codex").await).is_superset(&["cli", "desktop"].map(String::from).into()));
    let cursor = api.source("cursor").await;
    assert_eq!(cursor["captures"][0]["name"], "Cursor");
    assert!(cursor["cannot_capture"].get("Cursor CLI").is_some());
}

// --- broken spec files ---

#[tokio::test]
async fn a_broken_override_is_reported_on_the_bundled_connector_it_falls_back_to() {
    let api = api();
    std::fs::create_dir_all(api.settings.user_sources()).expect("mkdir");
    std::fs::write(api.settings.user_sources().join("copilot-cli.yaml"), "id: copilot-cli\n").expect("write");
    let found = api.source("copilot-cli").await;
    assert_eq!((&found["spec"]["origin"], &found["spec_problem"]["file"]["overrides"]), (&json!("bundled"), &json!(true)));
    assert!(found["spec_problem"]["error"].as_str().expect("error").contains("copilot-cli.yaml"));
    let problems = api.json("/api/sources/problems").await;
    assert_eq!(problems.as_array().map(Vec::len), Some(1));
    assert_eq!(problems[0]["fallback"], "copilot-cli");
}

#[tokio::test]
async fn a_broken_user_spec_is_isolated_and_every_route_keeps_working() {
    let api = api();
    std::fs::create_dir_all(api.settings.user_sources()).expect("mkdir");
    std::fs::write(api.settings.user_sources().join("mytool.yaml"), "id: mytool\n").expect("write"); // missing keys
    std::fs::write(api.settings.user_sources().join("other.yaml"), "id: [unclosed\n").expect("write"); // not YAML

    api.ingest("claude-code", hook_payload(&api.root())).await;
    let mut statuses = Vec::new();
    for route in ["/api/sources", "/api/prompts", "/api/fields", "/api/security", "/api/stats"] {
        statuses.push((route, api.get(route).send().await.status));
    }
    assert_eq!(statuses, [("/api/sources", 200), ("/api/prompts", 200), ("/api/fields", 200), ("/api/security", 200), ("/api/stats", 200)]);
    assert_eq!(api.json("/api/prompts").await.as_array().map(Vec::len), Some(1));

    let problems = api.json("/api/sources/problems").await;
    let names: Vec<&Value> = problems.as_array().expect("problems").iter().map(|p| &p["file"]["name"]).collect();
    assert_eq!(names, [&json!("mytool.yaml"), &json!("other.yaml")]);
    assert!(problems[0]["error"].as_str().expect("error").contains("mytool.yaml"));
    assert_eq!(problems[0]["file"]["origin"], "user");
}
