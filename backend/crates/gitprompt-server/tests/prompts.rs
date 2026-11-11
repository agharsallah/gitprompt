//! /api/prompts: the list with its filters, the detail with its turn, the user's notes and hiding
//! secrets. Port of `test_api.py` (events and telemetry enrich a prompt, origin and typed text) and
//! the prompt parts of `test_extension_points.py` / `test_stats.py`.
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
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const PID: &str = "550e8400-e29b-41d4-a716-446655440000";

fn api_request_export(prompt_id: &str) -> Value {
    otlp(
        json!({"service.name": "claude-code"}),
        json!({
            "event.name": "api_request",
            "prompt.id": prompt_id,
            "session.id": "abc123",
            "model": "claude-haiku",
            "input_tokens": {"intValue": "120"},
            "cost_usd": {"doubleValue": 0.002},
        }),
    )
}

/// A prompt with a secret, a long Write, a Stop and the telemetry of its model call.
async fn enriched(api: &common::Api) -> i64 {
    let base = hook_payload(&api.root());
    api.ingest("claude-code", with(&base, json!({"prompt": "ship it, password=hunter2hunter2"}))).await;
    let tool = json!({"hook_event_name": "PostToolUse", "tool_name": "Write", "tool_input": {"file_path": "/r/a.py", "content": "x".repeat(20_000)}, "duration_ms": 40});
    api.ingest("claude-code", with(&base, tool)).await;
    api.ingest("claude-code", with(&base, json!({"hook_event_name": "Stop", "last_assistant_message": "Done"}))).await;
    api.ingest_otlp("claude-code-otel", api_request_export(PID)).await;
    api.json("/api/prompts").await[0]["id"].as_i64().expect("id")
}

#[tokio::test]
async fn the_list_shows_what_the_turn_did_and_what_the_text_contains() {
    let api = api();
    enriched(&api).await;
    let listed = api.json("/api/prompts").await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    let prompt = &listed[0];
    assert_eq!(prompt["summary"]["tool_calls"], json!({"Write": 1}));
    assert_eq!(prompt["summary"]["input_tokens"], 120);
    assert_eq!(prompt["stats"]["secrets"], json!(["password or key assignment"]));
    assert_eq!(prompt["context"]["tool_version"], "2.1.316");
}

#[tokio::test]
async fn the_detail_lists_the_turns_events_oldest_first_with_long_values_clipped() {
    let api = api();
    let id = enriched(&api).await;
    let detail = api.json(&format!("/api/prompts/{id}")).await;
    assert_eq!(detail["summary"]["response"], "Done");
    assert_eq!(detail["summary"]["files_written"], json!(["/r/a.py"]));
    let events: Vec<&Value> = detail["events"].as_array().expect("events").iter().map(|e| &e["event"]).collect();
    assert_eq!(events, [&json!("PostToolUse"), &json!("Stop"), &json!("api_request")]);
    let content = detail["events"][0]["raw"]["tool_input"]["content"].as_str().expect("content");
    assert!(content.chars().count() < 10_100, "clipped: {} chars", content.chars().count());
    assert_eq!(detail["session"]["environment"], json!({"service.name": "claude-code"}));
}

#[tokio::test]
async fn the_detail_keeps_the_payload_exactly_as_received() {
    let api = api();
    let payload = hook_payload(&api.root());
    api.ingest("claude-code", payload.clone()).await;
    let prompt = &api.json("/api/prompts").await[0];
    assert_eq!(prompt["prompt"], payload["prompt"]);
    assert_eq!((&prompt["fields"]["session_id"], &prompt["fields"]["cwd"]), (&json!("abc123"), &payload["cwd"]));
    assert_eq!(api.json(&format!("/api/prompts/{}", prompt["id"])).await["raw"], payload);
}

#[tokio::test]
async fn an_unknown_prompt_is_404() {
    let api = api();
    let response = api.get("/api/prompts/999").send().await;
    assert_eq!((response.status, response.json()), (404, json!({"detail": "prompt not found"})));
}

#[tokio::test]
async fn the_text_query_keeps_prompts_with_every_word_ignoring_case() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    assert_eq!(api.json("/api/prompts?q=FACTORIAL").await.as_array().map(Vec::len), Some(1));
    assert_eq!(api.json("/api/prompts?q=factorial%20number").await.as_array().map(Vec::len), Some(1));
    assert_eq!(api.json("/api/prompts?q=nothing").await, json!([]));
}

#[tokio::test]
async fn the_limit_must_be_between_1_and_200() {
    let api = api();
    let statuses = [
        api.get("/api/prompts?limit=0").send().await.status,
        api.get("/api/prompts?limit=1").send().await.status,
        api.get("/api/prompts?limit=200").send().await.status,
        api.get("/api/prompts?limit=201").send().await.status,
        api.get("/api/prompts?limit=many").send().await.status,
    ];
    assert_eq!(statuses, [422, 200, 200, 422, 422]);
}

#[tokio::test]
async fn the_limit_keeps_the_newest_prompts() {
    let api = api();
    let base = hook_payload(&api.root());
    api.ingest("claude-code", with(&base, json!({"prompt_id": "a", "prompt": "first"}))).await;
    api.ingest("claude-code", with(&base, json!({"prompt_id": "b", "prompt": "second"}))).await;
    let listed = api.json("/api/prompts?limit=1").await;
    assert_eq!(listed.as_array().expect("list").iter().map(|p| p["prompt"].clone()).collect::<Vec<_>>(), [json!("second")]);
}

#[tokio::test]
async fn the_before_cursor_pages_to_older_prompts() {
    let api = api();
    let base = hook_payload(&api.root());
    api.ingest("claude-code", with(&base, json!({"prompt_id": "a", "prompt": "first"}))).await;
    api.ingest("claude-code", with(&base, json!({"prompt_id": "b", "prompt": "second"}))).await;
    let newest = api.json("/api/prompts?limit=1").await[0]["id"].clone();
    let older = api.json(&format!("/api/prompts?before={newest}")).await;
    assert_eq!(older.as_array().expect("list").iter().map(|p| p["prompt"].clone()).collect::<Vec<_>>(), [json!("first")]);
}

#[tokio::test]
async fn prompts_carry_origin_typed_text_and_injected_blocks() {
    let api = api();
    let base = hook_payload(&api.root());
    let typed = with(&base, json!({"prompt": "<ide_selection>x = 1</ide_selection>\nRename x"}));
    let notice = with(&base, json!({"prompt_id": "p2", "prompt": "<task-notification><task-id>t</task-id></task-notification>"}));
    api.ingest("claude-code", typed.clone()).await;
    api.ingest("claude-code", notice).await;

    let listed = api.json("/api/prompts").await;
    let (second, first) = (&listed[0], &listed[1]);
    assert_eq!((&first["origin"], &first["typed_text"], &first["prompt"]), (&json!("typed"), &json!("Rename x"), &typed["prompt"]));
    assert_eq!(first["injected"], json!([{"kind": "ide_selection", "chars": "<ide_selection>x = 1</ide_selection>".len()}]));
    assert_eq!((&second["origin"], &second["injected"]), (&json!("task_notification"), &json!([])));
    assert_eq!(api.json(&format!("/api/prompts/{}", first["id"])).await["origin"], "typed");
}

// --- the user's notes ---

#[tokio::test]
async fn an_annotation_is_saved_returned_and_filterable() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].clone();
    let note = json!({"starred": true, "rating": 1, "tags": ["release"], "note": "good one"});
    let saved = api.put(&format!("/api/prompts/{id}/annotation")).dashboard().json(note.clone()).send().await;
    assert_eq!((saved.status, saved.json()), (200, note.clone()));
    assert_eq!(api.json("/api/prompts?starred=true&tag=release").await[0]["annotation"], note);
    assert_eq!(api.json("/api/prompts?tag=other").await, json!([]));
}

#[tokio::test]
async fn a_bad_rating_round_trips_as_minus_one() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].clone();
    let note = json!({"starred": false, "rating": -1, "tags": [], "note": ""});
    api.put(&format!("/api/prompts/{id}/annotation")).dashboard().json(note.clone()).send().await;
    assert_eq!(api.json(&format!("/api/prompts/{id}")).await["annotation"], note);
}

#[tokio::test]
async fn a_rating_outside_minus_one_to_one_is_refused() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].clone();
    let response = api.put(&format!("/api/prompts/{id}/annotation")).dashboard().json(json!({"rating": 5})).send().await;
    assert_eq!(response.status, 422);
}

#[tokio::test]
async fn an_empty_annotation_body_resets_to_the_defaults() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].clone();
    let saved = api.put(&format!("/api/prompts/{id}/annotation")).dashboard().json(json!({})).send().await;
    assert_eq!(saved.json(), json!({"starred": false, "rating": 0, "tags": [], "note": ""}));
}

#[tokio::test]
async fn writes_to_a_missing_prompt_are_404() {
    let api = api();
    assert_eq!(api.put("/api/prompts/999/annotation").dashboard().json(json!({})).send().await.status, 404);
    assert_eq!(api.post("/api/prompts/999/redact").dashboard().send().await.status, 404);
}

// --- hiding secrets ---

#[tokio::test]
async fn redact_hides_the_secret_everywhere_it_was_stored() {
    let api = api();
    let id = enriched(&api).await;
    let response = api.post(&format!("/api/prompts/{id}/redact")).dashboard().send().await;
    assert_eq!((response.status, response.json()), (200, json!({"updated": 4})));
    let detail = api.get(&format!("/api/prompts/{id}")).send().await.text();
    assert!(!detail.contains("hunter2"), "secret still in the detail");
}

#[tokio::test]
async fn a_redacted_secret_is_no_longer_found_by_the_text_query() {
    let api = api();
    let payload =
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": "p9", "session_id": "s9", "prompt": "key password=hunter2hunter2"});
    api.ingest("claude-code", payload).await;
    let found = api.json("/api/prompts?q=hunter2hunter2").await;
    assert_eq!(found.as_array().map(Vec::len), Some(1));
    api.post(&format!("/api/prompts/{}/redact", found[0]["id"])).dashboard().send().await;
    assert_eq!(api.json("/api/prompts?q=hunter2hunter2").await, json!([]));
}
