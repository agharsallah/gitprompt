//! `GET /api/find`: prompts, sessions and projects for one query, grouped, finished prompts first,
//! end to end through ingest on a clock the test moves.
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

use gitprompt_server::ManualClock;
use jiff::SignedDuration;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::{Api, harness, project_id};

/// One turn of a session: where it ran, the session's title (once reported), the prompt, and the
/// event that ended it.
struct Turn<'a> {
    session: &'a str,
    cwd: &'a str,
    title: Option<&'a str>,
    prompt: &'a str,
    end: &'a str,
}

/// The prompt, then how it ended, each a minute after the previous event.
async fn play(api: &Api, clock: &ManualClock, turn: Turn<'_>) {
    let base = json!({"prompt_id": format!("p-{}", turn.prompt), "session_id": turn.session, "cwd": turn.cwd});
    let mut submit = common::with(&base, json!({"hook_event_name": "UserPromptSubmit", "prompt": turn.prompt}));
    if let Some(title) = turn.title {
        submit = common::with(&submit, json!({"session_title": title}));
    }
    clock.advance(SignedDuration::from_secs(60));
    api.ingest("claude-code", submit).await;
    clock.advance(SignedDuration::from_secs(60));
    api.ingest("claude-code", common::with(&base, json!({"hook_event_name": turn.end}))).await;
}

/// Two parser sessions in one folder (a failed prompt, newer, and a finished one, older) and one
/// session in another.
async fn data() -> Api {
    let clock = ManualClock::new("2026-03-02T09:00:00Z".parse().expect("instant"));
    let api = harness().moving_clock(&clock).start();
    let parser = "/work/parser";
    let turns = [
        Turn { session: "s1", cwd: parser, title: Some("Parser refactor"), prompt: "fix the parser crash", end: "Stop" },
        Turn { session: "s2", cwd: parser, title: None, prompt: "fix the parser crash again", end: "StopFailure" },
        Turn { session: "s3", cwd: "/work/site", title: Some("Site design"), prompt: "tweak the site header", end: "Stop" },
    ];
    for turn in turns {
        play(&api, &clock, turn).await;
    }
    api
}

fn ids(hits: &Value) -> Vec<&str> {
    hits.as_array().expect("hits").iter().map(|h| h["target"]["id"].as_str().expect("string id")).collect()
}

fn snippet_text(snippet: &Value) -> String {
    snippet.as_array().expect("segments").iter().map(|s| s["text"].as_str().expect("text")).collect()
}

fn titles(hits: &Value) -> Vec<&str> {
    hits.as_array().expect("hits").iter().map(|h| h["title"].as_str().unwrap_or_default()).collect()
}

#[tokio::test]
async fn a_word_finds_its_prompts_sessions_and_projects_grouped() {
    let api = data().await;
    let found = api.json("/api/find?q=parser").await;
    let prompts = found["prompts"].as_array().expect("prompts");
    let texts: Vec<String> = prompts.iter().map(|p| snippet_text(&p["snippet"])).collect();
    // the finished prompt first although the failed one is newer; the site prompt does not match
    assert_eq!(texts, ["fix the parser crash", "fix the parser crash again"]);
    assert_eq!(prompts[0]["snippet"][1], json!({"text": "parser", "hit": true}));
    assert_eq!(prompts.iter().map(|p| p["outcome"].clone()).collect::<Vec<_>>(), [json!("done"), json!("failed")]);
    assert_eq!(prompts[0]["target"]["kind"], "prompt");
    assert!(prompts[0]["target"]["id"].is_i64());
    assert_eq!(prompts[0]["title"], "Parser refactor");
    // s2 has no title: it is matched on the start of its first prompt
    assert_eq!(ids(&found["sessions"]), ["s2", "s1"]);
    assert_eq!(titles(&found["sessions"]), ["fix the parser crash again", "Parser refactor"]);
    assert_eq!(ids(&found["projects"]), [project_id("/work/parser")]);
    assert_eq!(titles(&found["projects"]), ["parser"]);
    assert_eq!(found["projects"][0]["target"]["kind"], "project");
}

#[tokio::test]
async fn an_empty_query_returns_the_most_recent_of_each_kind() {
    let api = data().await;
    for path in ["/api/find", "/api/find?q=", "/api/find?q=%20%20"] {
        let found = api.json(path).await;
        assert_eq!(ids(&found["sessions"]), ["s3", "s2", "s1"], "{path}");
        assert_eq!(ids(&found["projects"]), [project_id("/work/site"), project_id("/work/parser")], "{path}");
        let outcomes: Vec<&Value> = found["prompts"].as_array().expect("prompts").iter().map(|p| &p["outcome"]).collect();
        assert_eq!(outcomes, [&json!("done"), &json!("failed"), &json!("done")], "{path}: newest first, whatever the outcome");
    }
}

#[tokio::test]
async fn kinds_limit_which_lists_are_filled_and_limit_caps_each() {
    let api = data().await;
    for kinds in ["projects", "projects,projects"] {
        let found = api.json_with("/api/find", &[("kinds", kinds)]).await;
        assert_eq!((found["prompts"].clone(), found["sessions"].clone()), (json!([]), json!([])), "{kinds}");
        assert_eq!(found["projects"].as_array().expect("projects").len(), 2);
    }
    let found = api.json("/api/find?kinds=prompts&kinds=sessions&limit=1").await;
    assert_eq!((found["prompts"].as_array().unwrap().len(), found["sessions"].as_array().unwrap().len()), (1, 1));
    assert_eq!(found["projects"], json!([]));
}

#[tokio::test]
async fn a_query_nothing_matches_finds_nothing() {
    let api = data().await;
    assert_eq!(api.json("/api/find?q=zebra").await, json!({"prompts": [], "sessions": [], "projects": [], "commits": []}));
}

#[tokio::test]
async fn a_bad_kind_or_limit_is_a_422_naming_it() {
    let api = data().await;
    for (query, key) in [
        ("kinds=prompts,branches", "kinds"),
        ("kinds=prompt", "kinds"),
        ("limit=0", "limit"),
        ("limit=6", "limit"),
        ("limit=many", "limit"),
    ] {
        let response = api.get(&format!("/api/find?{query}")).send().await;
        assert_eq!(response.status, 422, "{query}");
        assert!(response.json()["detail"].as_str().expect("detail").starts_with(key), "{query}: {}", response.text());
    }
}

#[tokio::test]
async fn search_is_unchanged_beside_find() {
    let api = data().await;
    let search = api.json("/api/search?q=parser").await;
    assert_eq!(search["total"], 2);
}

#[tokio::test]
async fn a_session_you_renamed_is_found_and_called_by_its_name() {
    let api = data().await;
    let put = api.put("/api/names/session/s3").dashboard().json(json!({"name": "Landing page polish"})).send().await;
    assert_eq!(put.status, 200);

    let found = api.json("/api/find?q=landing&kinds=sessions").await;
    assert_eq!(ids(&found["sessions"]), ["s3"]);
    assert_eq!(titles(&found["sessions"]), ["Landing page polish"]);

    // the default label still matches, and the hit keeps your name
    let by_label = api.json("/api/find?q=site&kinds=sessions").await;
    assert_eq!(ids(&by_label["sessions"]), ["s3"]);
    assert_eq!(titles(&by_label["sessions"]), ["Landing page polish"]);
}

#[tokio::test]
async fn a_session_you_named_is_found_however_many_newer_sessions_there_are() {
    let clock = ManualClock::new("2026-03-02T09:00:00Z".parse().expect("instant"));
    let api = harness().moving_clock(&clock).start();
    play(&api, &clock, Turn { session: "old", cwd: "/work/site", title: None, prompt: "tweak the header", end: "Stop" }).await;
    let put = api.put("/api/names/session/old").dashboard().json(json!({"name": "Landing page polish"})).send().await;
    assert_eq!(put.status, 200);
    // more newer sessions than find reads before ranking
    for n in 0..205 {
        let (session, prompt) = (format!("new-{n}"), format!("unrelated work {n}"));
        play(&api, &clock, Turn { session: &session, cwd: "/work/other", title: None, prompt: &prompt, end: "Stop" }).await;
    }

    let found = api.json("/api/find?q=landing+polish&kinds=sessions").await;
    assert_eq!(ids(&found["sessions"]), ["old"]);
    assert_eq!(titles(&found["sessions"]), ["Landing page polish"]);
}
