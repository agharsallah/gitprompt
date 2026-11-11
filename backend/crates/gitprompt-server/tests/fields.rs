//! GET /api/fields: every field seen in stored payloads next to the fields each tool documents.
//! Port of the field-catalog assertions of `test_api.py`.
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

use common::{api, hook_payload, otlp, with};
use serde_json::{Value, json};

fn field<'a>(catalog: &'a Value, source: &str, event: &str, path: &str) -> &'a Value {
    catalog["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|f| f["source"] == source && f["event"] == event && f["path"] == path)
        .unwrap_or_else(|| panic!("no field {source} {event} {path}"))
}

#[tokio::test]
async fn the_catalog_marks_documented_seen_and_unseen_fields() {
    let api = api();
    let base = hook_payload(&api.root());
    api.ingest("claude-code", base.clone()).await;
    api.ingest(
        "claude-code",
        with(&base, json!({"hook_event_name": "PostToolUse", "tool_name": "Write", "tool_input": {"file_path": "/r/a.py"}})),
    )
    .await;
    let telemetry =
        otlp(json!({}), json!({"event.name": "api_request", "prompt.id": base["prompt_id"].clone(), "input_tokens": {"intValue": "120"}}));
    api.ingest_otlp("claude-code-otel", telemetry).await;

    let catalog = api.json("/api/fields").await;
    assert_eq!(field(&catalog, "claude-code", "PostToolUse", "tool_input.file_path")["documented"], true);
    assert_eq!(field(&catalog, "claude-code-otel", "api_request", "input_tokens")["count"], 1);
    assert_eq!(field(&catalog, "claude-code", "SessionEnd", "reason")["count"], 0); // documented, not seen yet
    assert!(catalog["not_captured"].as_array().expect("list").iter().any(|n| n["event"] == "SessionStart"));
}

#[tokio::test]
async fn the_catalog_documents_the_derived_values_origin_included() {
    let api = api();
    let derived = api.json("/api/fields").await["derived"].clone();
    assert!(derived.get("origin").and_then(Value::as_str).is_some_and(|d| !d.is_empty()), "{derived}");
}
