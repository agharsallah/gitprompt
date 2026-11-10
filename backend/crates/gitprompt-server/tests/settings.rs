//! /api/settings: each preference with its value and where it comes from; env-set ones are locked;
//! changes apply at once. Port of the preference tests of test_settings.py.

mod common;

use common::{api, harness, hook_payload, read_json, with};
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn each_setting_reports_its_value_default_source_and_variable() {
    let api = harness().env("GITPROMPT_WEEK_START", "sunday").start();
    let body = api.json("/api/settings").await;
    assert_eq!(body["file"], json!(api.settings.home.join("preferences.json").to_str()));
    assert_eq!(
        body["settings"]["default_range"],
        json!({"value": "7d", "default": "7d", "source": "default", "env": "GITPROMPT_DEFAULT_RANGE", "locked": false, "applies": "now"})
    );
    assert_eq!((&body["settings"]["week_start"]["source"], &body["settings"]["week_start"]["locked"]), (&json!("env"), &json!(true)));
}

#[tokio::test]
async fn every_preference_key_is_listed_in_order() {
    let api = api();
    let keys: Vec<String> = api.json("/api/settings").await["settings"].as_object().expect("settings").keys().cloned().collect();
    assert_eq!(keys, ["default_range", "week_start", "event_retention_days", "hide_secrets_on_ingest", "disabled_sources"]);
}

#[tokio::test]
async fn a_saved_change_is_reported_and_written_to_the_file() {
    let api = api();
    let saved = api.put("/api/settings").dashboard().json(json!({"default_range": "30d"})).send().await.json();
    assert_eq!((&saved["changed"], &saved["restart_required"]), (&json!(["default_range"]), &json!(false)));
    assert_eq!(saved["settings"]["default_range"]["source"], "file");
    assert_eq!(read_json(&api.settings.home.join("preferences.json")), json!({"default_range": "30d"}));
}

#[tokio::test]
async fn null_resets_a_preference_to_its_default() {
    let api = api();
    api.put("/api/settings").dashboard().json(json!({"default_range": "30d"})).send().await;
    let saved = api.put("/api/settings").dashboard().json(json!({"default_range": null})).send().await.json();
    assert_eq!(
        (&saved["settings"]["default_range"]["value"], &saved["settings"]["default_range"]["source"]),
        (&json!("7d"), &json!("default"))
    );
}

#[tokio::test]
async fn an_invalid_value_is_422_and_nothing_is_written() {
    let api = api();
    let response = api.put("/api/settings").dashboard().json(json!({"default_range": "30d", "week_start": "friday"})).send().await;
    assert_eq!(response.status, 422);
    assert!(!api.settings.home.join("preferences.json").exists());
}

#[tokio::test]
async fn an_out_of_range_value_or_an_unknown_key_is_422() {
    let api = api();
    assert_eq!(api.put("/api/settings").dashboard().json(json!({"default_range": "1y"})).send().await.status, 422);
    assert_eq!(api.put("/api/settings").dashboard().json(json!({"nope": 1})).send().await.status, 422);
}

#[tokio::test]
async fn a_setting_locked_by_env_is_409_naming_the_variable() {
    let api = harness().env("GITPROMPT_DEFAULT_RANGE", "24h").start();
    let locked = api.put("/api/settings").dashboard().json(json!({"default_range": "all"})).send().await;
    assert_eq!(locked.status, 409);
    assert!(locked.text().contains("GITPROMPT_DEFAULT_RANGE"), "{}", locked.text());
}

#[tokio::test]
async fn a_restart_is_reported_for_settings_that_apply_at_start() {
    let api = harness().applies_at_start(&["week_start"]).start();
    let saved = api.put("/api/settings").dashboard().json(json!({"week_start": "sunday"})).send().await.json();
    assert_eq!(saved["restart_required"], true);
}

#[tokio::test]
async fn hiding_secrets_on_ingest_applies_at_once_to_later_payloads_only() {
    let api = api();
    let secret = format!("AKIA{}", "ABCDEFGHIJKLMNOP");
    let base = hook_payload(&api.root());
    api.ingest("claude-code", with(&base, json!({"prompt": format!("use {secret}")}))).await;
    api.put("/api/settings").dashboard().json(json!({"hide_secrets_on_ingest": true})).send().await;
    api.ingest("claude-code", with(&base, json!({"prompt_id": "p2", "prompt": format!("again {secret}")}))).await;

    let listed = api.json("/api/prompts").await;
    let (newest, oldest) = (&listed[0], &listed[1]);
    assert_eq!(newest["prompt"], "again [hidden AWS access key]");
    assert!(!api.get(&format!("/api/prompts/{}", newest["id"])).send().await.text().contains(&secret)); // raw payload too
    assert!(oldest["prompt"].as_str().expect("prompt").contains(&secret)); // stored before the change
}
