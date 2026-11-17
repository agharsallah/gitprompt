//! How to pick a session up again, in the session list (the stats grouped by session) and the
//! session story: from the tool's spec and the folder the session started in, with one
//! ready-to-paste line; a tool whose spec has no template shows nothing. And when the session last
//! did anything, which tells one that is still going from one you left.
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

use common::{Api, api, harness};
use gitprompt_server::ManualClock;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A Claude Code session in a folder with a space and a quote, one in a plain folder, a Codex one,
/// and a Cursor one (its spec has no resume template).
async fn data() -> Api {
    let api = api();
    let claude = |session: &str, cwd: &str, event: &str| json!({"session_id": session, "prompt_id": format!("{session}-p"), "cwd": cwd, "hook_event_name": event, "prompt": "do the thing"});
    api.ingest("claude-code", claude("s-quoted", "/work/o'neil's app", "UserPromptSubmit")).await;
    api.ingest("claude-code", claude("s-plain", "/work/app", "UserPromptSubmit")).await;
    let codex =
        json!({"session_id": "s-codex", "turn_id": "t1", "cwd": "/work/api", "hook_event_name": "UserPromptSubmit", "prompt": "hi"});
    api.ingest("codex", codex).await;
    let cursor = json!({"conversation_id": "s-cursor", "generation_id": "g1", "hook_event_name": "beforeSubmitPrompt",
        "workspace_roots": ["/work/shop"], "prompt": "Add a coupon field"});
    api.ingest("cursor", cursor).await;
    api
}

/// The `resume` of each session of the list, by session id (`null` when omitted).
async fn listed(api: &Api) -> Value {
    let stats = api.json("/api/stats?group_by=session").await;
    Value::Object(
        stats["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| (g["key"].as_str().unwrap().to_string(), g.get("resume").cloned().unwrap_or(Value::Null)))
            .collect(),
    )
}

#[tokio::test]
async fn a_listed_session_carries_the_line_to_paste_with_its_folder_quoted_once() {
    let list = listed(&data().await).await;

    assert_eq!(
        list["s-plain"],
        json!({"command": "claude --resume s-plain", "cwd": "/work/app", "paste": "cd /work/app && claude --resume s-plain"})
    );
    assert_eq!(
        list["s-quoted"],
        json!({
            "command": "claude --resume s-quoted",
            "cwd": "/work/o'neil's app",
            "paste": r"cd '/work/o'\''neil'\''s app' && claude --resume s-quoted",
        }),
        "a space and quotes in the folder are quoted in `paste`; `cwd` stays as it is"
    );
    assert_eq!(list["s-codex"]["paste"], "cd /work/api && codex resume s-codex");
}

#[tokio::test]
async fn a_session_of_a_tool_without_a_template_has_no_resume() {
    let api = data().await;

    assert_eq!(listed(&api).await["s-cursor"], Value::Null, "omitted from the list");
    let story = api.json("/api/stories/session/s-cursor").await;
    assert!(story.get("resume").is_none(), "omitted from the story: {story}");
    assert_eq!(story["connector"]["label"], "Cursor", "the story still names the tool, so the page can say it has no command");
}

#[tokio::test]
async fn the_story_carries_the_same_resume_as_the_list() {
    let api = data().await;
    let list = listed(&api).await;

    for session in ["s-plain", "s-quoted", "s-codex"] {
        let story = api.json(&format!("/api/stories/session/{session}")).await;
        assert_eq!(story["resume"], list[session], "{session}");
    }
}

#[tokio::test]
async fn the_folder_is_the_first_one_any_event_of_the_session_reported() {
    let api = data().await;
    let event = |name: &str, cwd: &str| json!({"session_id": "s-moved", "prompt_id": "s-moved-p", "cwd": cwd, "hook_event_name": name, "prompt": "go"});
    api.ingest("claude-code", event("SessionStart", "/work/start")).await;
    api.ingest("claude-code", event("UserPromptSubmit", "/work/start/sub")).await;

    assert_eq!(listed(&api).await["s-moved"]["cwd"], "/work/start", "where it started, not where the prompt was typed");
}

#[tokio::test]
async fn a_session_says_when_it_last_did_anything_in_the_list_and_the_story() {
    let clock = ManualClock::new("2026-11-05T10:00:00Z".parse().unwrap());
    let api = harness().moving_clock(&clock).start();
    let event = |name: &str| json!({"session_id": "s1", "prompt_id": "p1", "cwd": "/work/app", "hook_event_name": name, "prompt": "go", "tool_name": "Read"});
    api.ingest("claude-code", event("UserPromptSubmit")).await;
    clock.advance(jiff::SignedDuration::from_secs(42));
    api.ingest("claude-code", event("PostToolUse")).await;
    clock.advance(jiff::SignedDuration::from_secs(600));

    let stats = api.json("/api/stats?group_by=session").await;
    let story = api.json("/api/stories/session/s1").await;

    assert_eq!(stats["groups"][0]["last_event_at"], "2026-11-05T10:00:42.000000+00:00", "the later event, not the prompt or now");
    assert_eq!(story["last_event_at"], stats["groups"][0]["last_event_at"]);
}
