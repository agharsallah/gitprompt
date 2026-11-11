//! `AppOptions::forwarder`: the desktop app passes its bundled `gitprompt` executable, and every
//! connect, status check and token rotation then uses the native forwarder form of each hook
//! command (`'<path>' hook <source> # /ingest/<source>`), which spools payloads while GitPrompt is
//! down. Without it (the default), the spec's curl commands.
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

use common::{Api, claude_settings, harness, read_json};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Every hook command GitPrompt installed in `~/.claude/settings.json`.
fn commands(api: &Api) -> Vec<String> {
    let written = read_json(&claude_settings(&api.settings));
    let mut out = Vec::new();
    for handlers in written["hooks"].as_object().expect("hooks").values() {
        for handler in handlers.as_array().expect("handlers").iter().flat_map(|h| h["hooks"].as_array().cloned().unwrap_or_default()) {
            out.extend(handler["command"].as_str().map(str::to_owned));
        }
    }
    out
}

fn sidecar(api_root: &std::path::Path) -> std::path::PathBuf {
    api_root.join("Applications/GitPrompt.app/Contents/MacOS/gitprompt")
}

#[tokio::test]
async fn with_a_forwarder_connect_installs_hooks_that_run_it_without_the_token() {
    let h = harness();
    let path = sidecar(&h.root());
    let api = h.forwarder(path.clone()).start();
    std::fs::write(claude_settings(&api.settings), "{}").expect("write");
    let applied = api.post("/api/sources/claude-code/connect").dashboard().send().await.json();
    assert_eq!(applied["applied"], true);
    let installed = commands(&api);
    let expected = format!("'{}' hook claude-code # /ingest/claude-code", path.display());
    assert!(!installed.is_empty() && installed.iter().all(|c| *c == expected), "{installed:?}");
    let written = std::fs::read_to_string(claude_settings(&api.settings)).expect("read");
    assert!(!written.contains(&api.token()), "the forwarder reads the token itself");
    let source = api.source("claude-code").await;
    assert_eq!((&source["connected"], &source["outdated"]), (&json!(true), &json!(false)));
}

#[tokio::test]
async fn without_a_forwarder_hooks_are_curl_commands() {
    let api = harness().start();
    std::fs::write(claude_settings(&api.settings), "{}").expect("write");
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    let installed = commands(&api);
    assert!(!installed.is_empty() && installed.iter().all(|c| c.starts_with("curl ")), "{installed:?}");
}

#[tokio::test]
async fn curl_hooks_are_outdated_for_an_app_with_a_forwarder_and_reconnecting_switches_them() {
    let h = harness();
    let path = sidecar(&h.root());
    let settings = h.settings.clone();
    let curl = h.start();
    std::fs::write(claude_settings(&settings), "{}").expect("write");
    curl.post("/api/sources/claude-code/connect").dashboard().send().await;

    let mut desktop = harness(); // the same data directory (kept alive by `curl`), as the desktop app
    desktop.settings = settings;
    let api = desktop.forwarder(path).start();
    assert_eq!(api.source("claude-code").await["outdated"], true);
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    assert!(commands(&api).iter().all(|c| c.contains(" hook claude-code ")), "{:?}", commands(&api));
    assert_eq!(api.source("claude-code").await["outdated"], false);
    drop(curl);
}

#[tokio::test]
async fn token_rotation_with_a_forwarder_keeps_the_forwarder_form() {
    let h = harness();
    let path = sidecar(&h.root());
    let api = h.forwarder(path).start();
    std::fs::write(claude_settings(&api.settings), "{}").expect("write");
    api.post("/api/sources/claude-code/connect").dashboard().send().await;
    let preview = api.post("/api/security/rotate-token").dashboard().send().await.json();
    // The hook commands carry no token: only the telemetry layer would change, and it isn't connected.
    assert_eq!(
        preview["reconnects"],
        json!([{"source": "claude-code", "name": "Claude Code", "change": {"file": claude_settings(&api.settings).to_str(), "diff": "", "applied": false, "backup": Value::Null}}])
    );
    let applied = api.post("/api/security/rotate-token?dry_run=false").dashboard().send().await.json();
    assert_eq!(applied["applied"], true);
    assert!(commands(&api).iter().all(|c| c.contains(" hook claude-code ")));
}
