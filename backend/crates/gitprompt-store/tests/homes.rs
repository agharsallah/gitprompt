//! Where a session ran (its tool and the folder it started in), for any number of sessions at once.
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
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use common::*;
use gitprompt_store::timeline::Home;
use pretty_assertions::assert_eq;
use serde_json::json;

fn ids(sessions: &[&str]) -> Vec<String> {
    sessions.iter().map(ToString::to_string).collect()
}

/// Where it ran and when it last did anything (the latest prompt is checked on its own).
fn home(source: &str, cwd: Option<&str>, last: &str) -> (Option<String>, Option<String>, Option<String>) {
    (Some(source.into()), cwd.map(str::to_owned), Some(last.into()))
}

fn place(home: &Home) -> (Option<String>, Option<String>, Option<String>) {
    (home.source.clone(), home.cwd.clone(), home.last_event_at.clone())
}

#[test]
fn a_session_runs_where_its_first_event_said_else_where_its_first_prompt_did() {
    let store = store();
    prompt("p1").session("s1").field("cwd", json!("/prompt/dir")).at("2026-11-03T10:00:00.000000+00:00").insert(&store);
    event("SessionStart").session("s1").field("cwd", json!("/event/dir")).at("2026-11-03T10:00:05.000000+00:00").insert(&store);
    event("PostToolUse").session("s1").field("cwd", json!("/later")).at("2026-11-03T10:00:09.000000+00:00").insert(&store);
    prompt("p2").session("s2").field("cwd", json!("/only/prompt")).source("codex").at("2026-11-03T10:00:00.000000+00:00").insert(&store);
    prompt("p3").session("s3").at("2026-11-03T10:00:00.000000+00:00").insert(&store);

    let homes = store.session_homes(&ids(&["s1", "s2", "s3", "unknown"])).unwrap();

    assert_eq!(
        place(&homes["s1"]),
        home("claude-code", Some("/event/dir"), "2026-11-03T10:00:09.000000+00:00"),
        "the last event is the later one"
    );
    assert_eq!(
        place(&homes["s2"]),
        home("codex", Some("/only/prompt"), "2026-11-03T10:00:00.000000+00:00"),
        "without events the prompt is the last thing"
    );
    assert_eq!(
        place(&homes["s3"]),
        home("claude-code", None, "2026-11-03T10:00:00.000000+00:00"),
        "no folder anywhere: the tool is still known"
    );
    assert_eq!(homes.get("unknown"), None, "a session without prompts has no home");
}

#[test]
fn an_empty_folder_does_not_count() {
    let store = store();
    event("SessionStart").session("s1").field("cwd", json!("")).insert(&store);
    prompt("p1").session("s1").field("cwd", json!("/real")).insert(&store);

    assert_eq!(store.session_homes(&ids(&["s1"])).unwrap()["s1"].cwd.as_deref(), Some("/real"));
}

#[test]
fn a_home_names_the_latest_prompt_and_the_latest_known_remote() {
    let store = store();
    let first =
        prompt("p1").session("s1").with(|r| r.repo_name = Some("acme/app".into())).at("2026-11-03T10:00:00.000000+00:00").insert(&store);
    let last = prompt("p2").session("s1").at("2026-11-03T10:05:00.000000+00:00").insert(&store);
    assert!(last > first);

    let home = &store.session_homes(&ids(&["s1"])).unwrap()["s1"];
    assert_eq!(home.latest_prompt, Some(last));
    assert_eq!(home.repo_name.as_deref(), Some("acme/app"), "a later prompt without a remote keeps the one before");
}

#[test]
fn the_whole_text_of_a_reused_prompt_is_found_by_its_hash() {
    let store = store();
    let long = "run just verify and fix what it finds ".repeat(10);
    prompt("p1").text(&long).with(|r| r.text_hash = Some("h1".into())).insert(&store);
    prompt("p2").text("other").with(|r| r.text_hash = Some("h2".into())).insert(&store);

    let texts = store.typed_texts(&ids(&["h1", "missing"])).unwrap();
    assert_eq!(texts.get("h1"), Some(&long));
    assert_eq!(texts.len(), 1, "a hash no prompt has is left out");
}
