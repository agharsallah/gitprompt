//! Snapshots of every read endpoint the dashboard calls, over the connector conformance fixtures.
//!
//! Every fixture of every tool (`backend/testdata/fixtures/<tool>/`) is ingested through the real
//! receiver, then each endpoint's JSON is compared with its snapshot in `tests/snapshots/`. Where
//! `ts_types.rs` keeps the dashboard's *types* in step with the Rust wire types, these keep the
//! *values*: a renamed, dropped or re-derived field shows up as a snapshot diff in review.
//!
//! Values that depend on the run (times, durations, the temp directory, the hourly heartbeat) are
//! replaced by placeholders, so the snapshots only change when the API does.
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

use serde_json::{Map, Value};

use common::{Api, api, project_id};

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

/// An app holding every fixture of every tool.
async fn loaded() -> Api {
    let api = api();
    for tool in TOOLS {
        for payload in payloads(tool) {
            api.ingest(tool, payload).await;
        }
    }
    api
}

// --- normalizing what depends on the run ---

/// Keys whose numbers are durations measured from arrival times.
fn is_duration(key: &str) -> bool {
    key.ends_with("_s") || key.contains("_s_p")
}

/// `2026-11-12T09:14:56.123456+00:00`, `...Z`: an ISO-8601 instant.
fn is_instant(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 20 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[..4].iter().all(u8::is_ascii_digit)
}

fn normalize(value: &mut Value, root: &str) {
    match value {
        Value::String(text) if is_instant(text) => *text = "[time]".into(),
        Value::String(text) if text.contains(root) => *text = text.replace(root, "[root]"),
        Value::Array(items) => items.iter_mut().for_each(|v| normalize(v, root)),
        Value::Object(map) => normalize_object(map, root),
        _ => {}
    }
}

fn normalize_object(map: &mut Map<String, Value>, root: &str) {
    for (key, value) in map.iter_mut() {
        match key.as_str() {
            "hourly" => *value = Value::String("[hourly]".into()),
            k if is_duration(k) && value.is_number() => *value = Value::String("[seconds]".into()),
            _ => normalize(value, root),
        }
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
