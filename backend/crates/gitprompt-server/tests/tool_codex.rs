//! OpenAI Codex CLI end to end through the API, data only (codex.yaml): connecting merges into the
//! user's own $`CODEX_HOME/hooks.json` and disconnecting restores it, TOML settings that turn hooks off
//! block connecting, and a turn's events join their prompt by `turn_id`. Port of `test_codex.py`.
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

use std::path::PathBuf;

use common::{Api, harness, read_json};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SESSION: &str = "0190a1b2-0000-7000-8000-000000000001";
const TURN: &str = "0190a1b2-0000-7000-8000-0000000000a1";

/// An app whose environment points `CODEX_HOME` at a temp folder.
fn codex_api() -> (Api, PathBuf) {
    let h = harness();
    let home = h.root().join("codex-home");
    std::fs::create_dir_all(&home).expect("mkdir");
    let api = h.env("CODEX_HOME", home.to_str().expect("utf-8")).start();
    (api, home)
}

fn user_hooks() -> Value {
    json!({"description": "mine", "hooks": {"PreToolUse": [{"matcher": "^Bash$", "hooks": [{"type": "command", "command": "true"}]}]}})
}

fn codex(event: &str, extra: Value) -> Value {
    let mut payload =
        json!({"session_id": SESSION, "transcript_path": "/t.jsonl", "cwd": "/work/app", "model": "gpt-5-codex", "hook_event_name": event});
    payload.as_object_mut().expect("object").extend(extra.as_object().expect("object").clone());
    payload
}

#[tokio::test]
async fn codex_is_detected_in_codex_home_with_its_config_file_and_assistant() {
    let (api, home) = codex_api();
    std::fs::write(home.join("hooks.json"), "{}").expect("write");
    let found = api.source("codex").await;
    assert_eq!(
        (&found["detected"], &found["config_file"], &found["assistant"]),
        (&json!(true), &json!(home.join("hooks.json").to_str()), &json!("Codex"))
    );
    assert!(found["after_connect"].as_str().expect("after_connect").contains("/hooks"));
}

#[tokio::test]
async fn connect_merges_into_the_users_hooks_file_and_disconnect_restores_it() {
    let (api, home) = codex_api();
    let hooks = home.join("hooks.json");
    let before = format!("{}\n", serde_json::to_string_pretty(&user_hooks()).expect("json"));
    std::fs::write(&hooks, &before).expect("write");

    assert_eq!(api.post("/api/sources/codex/connect").dashboard().send().await.json()["applied"], true);
    let written = read_json(&hooks);
    assert_eq!((&written["description"], &written["hooks"]["PreToolUse"]), (&json!("mine"), &user_hooks()["hooks"]["PreToolUse"]));
    let prompt_hook = &written["hooks"]["UserPromptSubmit"][0]["hooks"][0];
    let end_hook = &written["hooks"]["SessionEnd"][0]["hooks"][0];
    let command = prompt_hook["command"].as_str().expect("command");
    assert_eq!(prompt_hook["async"], true);
    assert!(command.contains("/ingest/codex") && !command.contains("{{"), "{command}");
    assert!(end_hook.get("async").is_none() && end_hook["timeout"].as_f64().is_some_and(|t| t <= 3.0), "{end_hook}"); // SessionEnd runs synchronously, 3 s at most
    assert!(written["hooks"]["Stop"][0]["hooks"][0].get("async").is_none()); // an async Stop is cancelled when `codex exec` ends
    assert_eq!(api.source("codex").await["connected"], true);

    api.post("/api/sources/codex/disconnect").dashboard().send().await;
    assert_eq!(std::fs::read_to_string(&hooks).expect("read"), before);
    assert_eq!(api.source("codex").await["connected"], false);
}

async fn blocked_by(config: &str) -> (Value, u16) {
    let (api, home) = codex_api();
    std::fs::write(home.join("config.toml"), config).expect("write");
    let blocks = api.source("codex").await["blocks"].clone();
    (blocks, api.post("/api/sources/codex/connect").dashboard().send().await.status)
}

#[tokio::test]
async fn toml_features_hooks_false_blocks_connecting() {
    let (blocks, status) = blocked_by("[features]\nhooks = false\n").await;
    assert_ne!(blocks, json!([]));
    assert_eq!(status, 409);
}

#[tokio::test]
async fn toml_features_codex_hooks_false_blocks_connecting() {
    let (blocks, status) = blocked_by("[features]\ncodex_hooks = false\n").await;
    assert_ne!(blocks, json!([]));
    assert_eq!(status, 409);
}

#[tokio::test]
async fn unrelated_toml_does_not_block() {
    let (blocks, _) = blocked_by("model = \"gpt-5-codex\"\n[features]\nhooks = true\n").await;
    assert_eq!(blocks, json!([]));
}

#[tokio::test]
async fn broken_toml_does_not_block_because_codex_cannot_apply_it_either() {
    let (blocks, _) = blocked_by("[features\nhooks = false\n").await;
    assert_eq!(blocks, json!([]));
}

/// A turn as Codex sends it: hooks run async, so a tool result can arrive before its prompt.
async fn one_turn(api: &Api) {
    let patch = "*** Begin Patch\n*** Update File: /work/app/upload.py\n@@\n-a\n+b\n*** End Patch\n";
    let first_tool =
        json!({"turn_id": TURN, "tool_name": "apply_patch", "tool_use_id": "c1", "tool_input": {"command": patch}, "tool_response": "ok"});
    api.ingest("codex", codex("PostToolUse", first_tool)).await;
    api.ingest(
        "codex",
        codex("UserPromptSubmit", json!({"turn_id": TURN, "permission_mode": "default", "prompt": "Add a retry to the upload client"})),
    )
    .await;
    api.ingest("codex", codex("PostToolUse", json!({"turn_id": TURN, "tool_name": "Bash", "tool_use_id": "c2", "tool_input": {"command": "pytest -q"}, "tool_response": "ok"}))).await;
    api.ingest("codex", codex("Stop", json!({"turn_id": TURN, "stop_hook_active": false, "last_assistant_message": "Added a retry."})))
        .await;
}

#[tokio::test]
async fn a_turn_joins_its_prompt_by_turn_id_even_when_a_tool_result_arrives_first() {
    let (api, _) = codex_api();
    one_turn(&api).await;
    let listed = api.json("/api/prompts?source=codex").await;
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    let prompt = &listed[0];
    assert_eq!((&prompt["fields"]["prompt_id"], &prompt["origin"]), (&json!(TURN), &json!("typed")));
    let summary = &prompt["summary"];
    assert_eq!((&summary["tool_kinds"], &summary["files_written"]), (&json!({"write": 1, "shell": 1}), &json!(["/work/app/upload.py"])));
    assert_eq!((&summary["response"], &summary["ended_by"]), (&json!("Added a retry."), &json!("Stop")));
}

#[tokio::test]
async fn a_codex_turn_counts_its_hook_reported_model_without_usage() {
    let (api, _) = codex_api();
    one_turn(&api).await;
    let groups = api.json("/api/stats?source=codex&group_by=model").await["groups"].clone();
    assert_eq!(groups.as_array().map(Vec::len), Some(1));
    assert_eq!(
        (&groups[0]["key"], common::num(&groups[0]["prompts"]), common::num(&groups[0]["api_requests"])),
        (&json!("gpt-5-codex"), 1.0, 0.0)
    );
    let story = api.json(&format!("/api/stories/session/{SESSION}")).await;
    assert_eq!(story["items"][0]["files"], json!(["/work/app/upload.py"]));
}

#[tokio::test]
async fn an_overview_without_reported_usage_leaves_cost_and_tokens_out() {
    let (api, _) = codex_api();
    one_turn(&api).await;
    let overview = api.get("/api/export/overview?source=codex&format=md").send().await.text();
    assert!(overview.contains("**Prompts:** 1") && overview.contains("gpt-5-codex"), "{overview}");
    assert!(!overview.contains("**Cost:**") && !overview.contains("**Tokens:**"), "{overview}"); // left out, not $0
}
