//! `GET /api/attention`: the sessions to pick up, whatever the range, and what is worth a look in
//! it; time from the app's clock.
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

use common::{Api, harness};
use gitprompt_server::ManualClock;
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const MIN: SignedDuration = SignedDuration::from_mins(1);

fn at(text: &str) -> Timestamp {
    text.parse().expect("instant")
}

async fn prompt(api: &Api, prompt_id: &str, session: &str, text: &str) {
    let payload =
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": prompt_id, "session_id": session, "prompt": text, "cwd": "/r"});
    api.ingest("claude-code", payload).await;
}

async fn stop(api: &Api, prompt_id: &str, session: &str) {
    api.ingest("claude-code", json!({"hook_event_name": "Stop", "prompt_id": prompt_id, "session_id": session})).await;
}

fn sessions(feed: &Value) -> Vec<(String, String)> {
    let rows = feed["pick_up"].as_array().unwrap();
    rows.iter().map(|r| (r["session"].as_str().unwrap().to_owned(), r["presence"].as_str().unwrap().to_owned())).collect()
}

#[tokio::test]
async fn an_empty_store_has_nothing_that_needs_you() {
    let api = harness().start();
    assert_eq!(api.json("/api/attention").await, json!({"pick_up": [], "worth_a_look": [], "nothing_needs_you": true}));
}

#[tokio::test]
async fn a_running_session_comes_before_one_you_left_and_flips_once_quiet() {
    let clock = ManualClock::new(at("2026-11-18T08:00:00Z"));
    let api = harness().moving_clock(&clock).start();
    prompt(&api, "p1", "left", "fix the parser").await;
    stop(&api, "p1", "left").await;
    clock.advance(30 * MIN);
    prompt(&api, "p2", "busy", "write the docs").await;

    let feed = api.json("/api/attention").await;
    assert_eq!(sessions(&feed), [("busy".into(), "running".into()), ("left".into(), "left".into())]);
    assert_eq!(feed["pick_up"][1]["ended"], "done");
    assert_eq!(feed["pick_up"][1]["resume"]["command"], "claude --resume left");
    assert_eq!(feed["nothing_needs_you"], false);

    clock.advance(11 * MIN);
    assert_eq!(sessions(&api.json("/api/attention").await)[0], ("busy".into(), "left".into()));
}

#[tokio::test]
async fn worth_a_look_follows_the_range_but_pick_up_does_not() {
    let clock = ManualClock::new(at("2026-11-10T08:00:00Z"));
    let api = harness().moving_clock(&clock).start();
    for n in 0..6 {
        clock.advance(MIN);
        prompt(&api, &format!("p{n}"), &format!("s{}", n % 2), "run just verify and fix").await;
    }
    clock.set(at("2026-11-12T08:00:00Z"));

    let all = api.json("/api/attention").await;
    assert_eq!(all["worth_a_look"][0]["kind"], "repeated_prompt");
    assert_eq!(
        all["worth_a_look"][0]["action"],
        json!({"type": "copy_command", "name": "run-just-verify-and", "text": "run just verify and fix"})
    );
    assert_eq!(all["worth_a_look"][0]["target"], json!({"page": "prompts", "q": "run just verify and fix"}));

    let today = api.json_with("/api/attention", &[("since", "2026-11-12T00:00:00Z")]).await;
    assert_eq!(today["worth_a_look"], json!([]), "nothing repeated today");
    assert_eq!(sessions(&today).len(), 2, "the sessions to pick up are the latest, whatever the range");
}
