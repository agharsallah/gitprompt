//! `Service::glance`: what the desktop tray shows. Today counts from local midnight on the app's
//! clock; the instants are chosen so the machine's timezone cannot move them across it (a prompt
//! a minute ago is today anywhere, one 30 hours ago is before today's midnight anywhere).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
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

const NOW: &str = "2026-11-18T12:00:00Z";

fn at(text: &str) -> Timestamp {
    text.parse().expect("instant")
}

fn before(hours: i64) -> Timestamp {
    at(NOW) - SignedDuration::from_hours(hours)
}

async fn prompt(api: &Api, id: &str, text: &str) {
    api.ingest(
        "claude-code",
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": id, "session_id": "s", "prompt": text, "cwd": "/r"}),
    )
    .await;
}

async fn cost(api: &Api, id: &str, usd: f64) {
    api.ingest(
        "claude-code",
        json!({"hook_event_name": "api_request", "prompt_id": id, "session_id": "s", "model": "opus", "cost_usd": usd}),
    )
    .await;
}

async fn decide(api: &Api, id: &str, state: Value) {
    let saved = api.put(&format!("/api/leaks/{id}")).dashboard().json(json!({ "state": state })).send().await;
    assert_eq!(saved.status, 200, "{}", saved.text());
}

#[tokio::test]
async fn today_counts_from_local_midnight_and_has_no_cost_when_none_was_reported() {
    let clock = ManualClock::new(before(30));
    let api = harness().moving_clock(&clock).start();
    prompt(&api, "old", "yesterday's work").await;
    cost(&api, "old", 4.0).await;
    clock.set(before(0) - SignedDuration::from_mins(1));
    prompt(&api, "p1", "today's work").await;
    clock.set(at(NOW));

    let glance = api.service().glance().await.unwrap();
    assert_eq!((glance.today.prompts, glance.today.cost_usd), (1, None), "yesterday's cost is not today's");

    cost(&api, "p1", 0.5).await;
    assert_eq!(api.service().glance().await.unwrap().today.cost_usd, Some(0.5));
}

#[tokio::test]
async fn the_last_three_typed_prompts_newest_first() {
    let api = harness().clock(at(NOW)).start();
    for (id, text) in [("p1", "one"), ("p2", "two"), ("p3", "three"), ("p4", "four")] {
        prompt(&api, id, text).await;
    }
    let recent: Vec<String> = api.service().glance().await.unwrap().recent.into_iter().map(|p| p.text).collect();
    assert_eq!(recent, ["four", "three", "two"]);
}

#[tokio::test]
async fn open_leaks_count_until_handled() {
    let api = harness().clock(at(NOW)).start();
    prompt(&api, "p1", &format!("use {} now", "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd")).await;
    prompt(&api, "p2", "call me on +49 30 1234567 tomorrow").await;
    assert_eq!(api.service().glance().await.unwrap().open_leaks, 2);

    let open = api.json("/api/leaks").await["open"].clone();
    decide(&api, open[0]["id"].as_str().unwrap(), json!("hidden")).await;
    decide(&api, open[1]["id"].as_str().unwrap(), json!("not_sensitive")).await;
    assert_eq!(api.service().glance().await.unwrap().open_leaks, 0);
}

#[tokio::test]
async fn sessions_to_pick_up_are_the_overviews() {
    let api = harness().clock(at(NOW)).start();
    prompt(&api, "p1", "go").await;
    let glance = api.service().glance().await.unwrap();
    let feed = api.json("/api/attention").await;
    assert_eq!(serde_json::to_value(&glance.pick_up).unwrap(), feed["pick_up"]);
    assert_eq!(glance.pick_up[0].session, "s");
}
