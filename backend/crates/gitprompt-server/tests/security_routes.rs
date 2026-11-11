//! /api/security: the receiver token, and rotating it (which reconnects every connected source,
//! with the same dry-run diff as Connect). Port of the security tests of `test_settings.py`.
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

use std::os::unix::fs::PermissionsExt;

use common::{Api, api, claude_settings, read_json};
use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::json;

fn connected_claude(api: &Api) -> std::path::PathBuf {
    let path = claude_settings(&api.settings);
    std::fs::write(&path, "{}").expect("write");
    path
}

#[tokio::test]
async fn security_names_the_token_file_when_it_changed_and_the_connected_sources() {
    let api = api();
    connected_claude(&api);
    let before = api.json("/api/security").await;
    assert_eq!((&before["token_file"], &before["connected"]), (&json!(api.settings.token_file().to_str()), &json!([])));
    assert!(before["token_changed"].as_str().expect("time").parse::<Timestamp>().is_ok(), "{}", before["token_changed"]);
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    assert_eq!(api.json("/api/security").await["connected"], json!(["Claude Code"]));
}

#[tokio::test]
async fn a_rotation_dry_run_shows_each_reconnect_and_changes_nothing() {
    let api = api();
    let path = connected_claude(&api);
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    let old = api.token();
    let preview = api.post("/api/security/rotate-token").dashboard().send().await.json();
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["reconnects"].as_array().map(Vec::len), Some(1));
    assert_eq!(preview["reconnects"][0]["source"], "claude-code");
    assert!(preview["reconnects"][0]["change"]["diff"].as_str().expect("diff").contains("<new token>"));
    assert_eq!(api.token(), old);
    assert!(std::fs::read_to_string(&path).expect("read").contains(&old));
}

#[tokio::test]
async fn an_applied_rotation_rewrites_the_token_and_every_connected_config() {
    let api = api();
    let path = connected_claude(&api);
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    let old = api.token();
    let applied = api.post("/api/security/rotate-token?dry_run=false").dashboard().send().await.json();
    let new = api.token();
    assert_eq!(applied["applied"], true);
    assert_ne!(new, old);
    assert_eq!(std::fs::metadata(api.settings.token_file()).expect("stat").permissions().mode() & 0o777, 0o600);
    let written = std::fs::read_to_string(&path).expect("read");
    assert!(written.contains(&new) && !written.contains(&old));
    assert!(applied["reconnects"][0]["change"]["backup"].is_string());
}

#[tokio::test]
async fn the_old_token_is_refused_right_after_a_rotation() {
    let api = api();
    let old = api.token();
    api.post("/api/security/rotate-token?dry_run=false").dashboard().send().await;
    let stale = api.post("/ingest/claude-code").bearer(&old).json(json!({"hook_event_name": "Stop"})).send().await;
    assert_eq!(stale.status, 401);
    let fresh = api.post("/ingest/claude-code").auth().json(json!({"hook_event_name": "Stop"})).send().await;
    assert_eq!(fresh.status, 204);
}

#[tokio::test]
async fn rotation_is_refused_when_a_connected_source_cannot_be_reconnected() {
    let api = api();
    let path = connected_claude(&api);
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    let mut doc = read_json(&path);
    doc["disableAllHooks"] = json!(true);
    std::fs::write(&path, doc.to_string()).expect("write");
    let old = api.token();

    let preview = api.post("/api/security/rotate-token").dashboard().send().await.json();
    assert_ne!(preview["blocked"], json!([]));
    assert_eq!(preview["reconnects"], json!([]));
    assert_eq!(api.post("/api/security/rotate-token?dry_run=false").dashboard().send().await.status, 409);
    assert_eq!(api.token(), old);
}
