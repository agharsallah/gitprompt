//! Snapshots of every read endpoint the dashboard calls, over the connector conformance fixtures.
//!
//! Every fixture of every tool (`backend/testdata/fixtures/<tool>/`) is ingested through the real
//! receiver, then each endpoint's JSON is compared with its snapshot in `tests/snapshots/`. Where
//! `ts_types.rs` keeps the dashboard's *types* in step with the Rust wire types, these keep the
//! *values*: a renamed, dropped or re-derived field shows up as a snapshot diff in review.
//!
//! Time is not redacted but controlled: the app runs on a [`ManualClock`] that starts at [`START`]
//! and moves [`STEP`] before each fixture, so arrival times, the durations between them and the
//! hourly heartbeat are exact values here, the same on every machine and every day. Only the temp
//! directory is replaced (by `[root]`). Never read the system clock on a path these snapshots
//! cover: take "now" from the app's `Clock` (`gitprompt_server::clock`).
//!
//! Update after an intended change: `just snapshots`, then review the diff of `tests/snapshots/`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use std::path::{Path, PathBuf};

use gitprompt_server::ManualClock;
use jiff::SignedDuration;
use serde_json::Value;

use common::{Api, harness, project_id};

/// The tools whose fixtures are ingested, in this order (ids follow it).
const TOOLS: &[&str] = &["claude-code", "codex", "copilot-cli", "cursor"];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/fixtures")
}

/// The payloads of one tool's fixtures, in file-name order.
fn payloads(tool: &str) -> Vec<Value> {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(fixtures_dir().join(tool)).expect("fixtures dir").map(|e| e.expect("entry").path()).collect();
    files.sort();
    files
        .iter()
        .filter(|f| f.extension().is_some_and(|e| e == "json"))
        .map(|f| {
            let fixture: Value = serde_json::from_str(&std::fs::read_to_string(f).expect("read fixture")).expect("fixture JSON");
            fixture["payload"].clone()
        })
        .collect()
}

/// When the first fixture arrives, and the time between two arrivals.
const START: &str = "2026-01-05T09:00:00Z";
const STEP: SignedDuration = SignedDuration::from_secs(7);

/// An app holding every fixture of every tool, each arriving [`STEP`] after the previous one on a
/// clock the test moves, so every `received_at` and duration between arrivals is the same each run.
async fn loaded() -> Api {
    let clock = ManualClock::new(START.parse().expect("instant"));
    let api = harness().moving_clock(&clock).start();
    for tool in TOOLS {
        for payload in payloads(tool) {
            clock.advance(STEP);
            api.ingest(tool, payload).await;
        }
    }
    api
}

// --- normalizing what depends on the run ---

/// The temp directory is the only value that differs between runs.
fn normalize(value: &mut Value, root: &str) {
    match value {
        Value::String(text) if text.contains(root) => *text = text.replace(root, "[root]"),
        Value::Array(items) => items.iter_mut().for_each(|v| normalize(v, root)),
        Value::Object(map) => map.values_mut().for_each(|v| normalize(v, root)),
        _ => {}
    }
}

/// GET `path` (must answer 200), normalized.
async fn get(api: &Api, path: &str) -> Value {
    let mut body = api.json(path).await;
    normalize(&mut body, &api.root().to_string_lossy());
    body
}

/// The first prompt `source` sent (the list is newest first).
fn first_of<'a>(prompts: &'a Value, source: &str) -> &'a Value {
    prompts.as_array().expect("list").iter().rfind(|p| p["source"] == source).unwrap_or_else(|| panic!("no {source} prompt"))
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter().fold(value, |v, k| &v[*k]).as_str().unwrap_or_else(|| panic!("no text at {path:?}"))
}

// --- the snapshots ---

#[tokio::test]
async fn prompts_and_their_details() {
    let api = loaded().await;
    let prompts = get(&api, "/api/prompts?limit=200").await;
    insta::assert_json_snapshot!("prompts", prompts);
    for tool in TOOLS {
        let id = first_of(&prompts, tool)["id"].as_i64().expect("id");
        insta::assert_json_snapshot!(format!("prompt_detail_{tool}"), get(&api, &format!("/api/prompts/{id}")).await);
    }
}

#[tokio::test]
async fn stats_and_telemetry() {
    let api = loaded().await;
    insta::assert_json_snapshot!("stats", get(&api, "/api/stats").await);
    for group in ["source", "origin", "outcome", "model", "tool_kind"] {
        insta::assert_json_snapshot!(format!("stats_by_{group}"), get(&api, &format!("/api/stats?group_by={group}")).await);
    }
    insta::assert_json_snapshot!("telemetry", get(&api, "/api/stats/telemetry").await);
}

#[tokio::test]
async fn stories_lineage_and_search() {
    let api = loaded().await;
    let prompts = api.json("/api/prompts?limit=200").await;
    for tool in TOOLS {
        let prompt = first_of(&prompts, tool);
        let session = text(prompt, &["fields", "session_id"]);
        insta::assert_json_snapshot!(format!("session_story_{tool}"), get(&api, &format!("/api/stories/session/{session}")).await);
        let id = prompt["id"].as_i64().expect("id");
        insta::assert_json_snapshot!(format!("lineage_{tool}"), get(&api, &format!("/api/stories/prompt/{id}")).await);
    }
    let project = project_id(Path::new("/work/app").to_str().expect("utf-8"));
    insta::assert_json_snapshot!("project_story", get(&api, &format!("/api/stories/project/{project}")).await);
    insta::assert_json_snapshot!("search", get(&api, "/api/search?q=retry").await);
}

#[tokio::test]
async fn connectors_fields_and_settings() {
    let api = loaded().await;
    insta::assert_json_snapshot!("sources", get(&api, "/api/sources").await);
    insta::assert_json_snapshot!("sources_recent", get(&api, "/api/sources/recent").await);
    insta::assert_json_snapshot!("sources_problems", get(&api, "/api/sources/problems").await);
    insta::assert_json_snapshot!("fields", get(&api, "/api/fields").await);
    insta::assert_json_snapshot!("settings", get(&api, "/api/settings").await);
}
