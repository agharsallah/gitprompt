//! `GET /api/leaks` and `PUT /api/leaks/{id}`: findings masked, hidden in the stored text, or
//! dismissed wherever the same value appears; the clear value is never in an answer.
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

use common::{Api, api};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Synthetic, built at run time so no file holds a key-shaped string.
fn key() -> String {
    "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd"
}

async fn prompt(api: &Api, prompt_id: &str, text: &str) {
    api.ingest("claude-code", json!({"hook_event_name": "UserPromptSubmit", "prompt_id": prompt_id, "session_id": "s", "prompt": text}))
        .await;
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap().to_owned()).collect()
}

async fn decide(api: &Api, id: &str, state: Value) -> common::Resp {
    api.put(&format!("/api/leaks/{id}")).dashboard().json(json!({ "state": state })).send().await
}

#[tokio::test]
async fn a_finding_is_listed_masked_with_what_was_checked() {
    let api = api();
    prompt(&api, "p1", &format!("use {} please", key())).await;
    let board = api.json("/api/leaks").await;
    assert_eq!(board["open"][0]["kind"], "OpenAI API key");
    assert_eq!(board["open"][0]["masked"], "sk-…y6Hd");
    assert_eq!(board["open"][0]["place"], "prompt");
    assert_eq!(board["checked"]["prompts"], 1);
    assert!(!board.to_string().contains("q8Zt2Lw9XkP4vR7mN3cB"), "never the clear value");
}

#[tokio::test]
async fn not_sensitive_dismisses_the_value_everywhere_and_can_be_undone() {
    let api = api();
    prompt(&api, "p1", &format!("use {}", key())).await;
    prompt(&api, "p2", &format!("again {}", key())).await;
    let open = ids(&api.json("/api/leaks").await["open"]);
    assert_eq!(open.len(), 2);

    let saved = decide(&api, &open[0], json!("not_sensitive")).await;
    assert_eq!(saved.status, 200, "{}", saved.text());
    let board = api.json("/api/leaks").await;
    assert_eq!((board["open"].as_array().unwrap().len(), ids(&board["dismissed"]).len()), (0, 2), "the same value in another prompt too");

    assert_eq!(decide(&api, &open[0], Value::Null).await.status, 200);
    assert_eq!(ids(&api.json("/api/leaks").await["open"]).len(), 2);
}

#[tokio::test]
async fn hidden_replaces_the_value_in_what_the_reader_gets_and_stays_listed() {
    let api = api();
    prompt(&api, "p1", &format!("use {} please", key())).await;
    let id = ids(&api.json("/api/leaks").await["open"])[0].clone();

    let saved = decide(&api, &id, json!("hidden")).await;
    assert_eq!(saved.status, 200, "{}", saved.text());
    assert!(saved.json()["updated"].as_u64().unwrap() >= 1);

    let prompts = api.json("/api/prompts").await;
    let detail = api.json(&format!("/api/prompts/{}", prompts[0]["id"])).await;
    assert!(!detail.to_string().contains("q8Zt2Lw9XkP4vR7mN3cB"), "the value is gone from the stored text");
    assert!(detail.to_string().contains("[hidden OpenAI API key]"));
    let board = api.json("/api/leaks").await;
    assert_eq!((ids(&board["open"]).len(), ids(&board["hidden"])), (0, vec![id]));
    assert_eq!(board["hidden"][0]["masked"], "sk-…y6Hd");
}

#[tokio::test]
async fn an_unknown_finding_a_bad_body_and_a_write_without_the_header_are_refused() {
    let api = api();
    assert_eq!(decide(&api, "0123456789abcdef", json!("not_sensitive")).await.status, 404);
    assert_eq!(decide(&api, "0123456789abcdef", Value::Null).await.status, 404);
    assert_eq!(decide(&api, "0123456789abcdef", json!("maybe")).await.status, 422);
    let bare = api.put("/api/leaks/0123456789abcdef").json(json!({"state": "hidden"})).send().await;
    assert_eq!(bare.status, 403);
}

#[tokio::test]
async fn a_value_on_the_allow_list_is_never_shown() {
    let api = api();
    prompt(&api, "p1", "mail marie.curie@dupont.io about it").await;
    assert_eq!(ids(&api.json("/api/leaks").await["open"]).len(), 1);
    let saved = api.put("/api/settings").dashboard().json(json!({"leak_allow": ["Marie.Curie@dupont.io"]})).send().await;
    assert_eq!(saved.status, 200, "{}", saved.text());
    assert_eq!(api.json("/api/leaks").await["open"], json!([]));
}

#[tokio::test]
async fn open_findings_lead_worth_a_look_and_count_in_the_key_figures() {
    let api = api();
    prompt(&api, "p1", &format!("use {}", key())).await;
    let feed = api.json("/api/attention").await;
    assert_eq!(feed["worth_a_look"][0]["kind"], "possible_leak");
    assert_eq!(feed["worth_a_look"][0]["action"]["type"], "hide_values");
    assert_eq!(feed["worth_a_look"][0]["target"], json!({"page": "leaks"}));
    assert_eq!(api.json_with("/api/stats", &[("key_figures", "true")]).await["possible_leaks"], 1);
}
