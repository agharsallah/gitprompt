//! `GET /api/stats?previous=true`: the totals of the period of equal length before `since`, and
//! each key figure against it. Time comes from a `ManualClock`, so the edges are exact instants.
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

const WEEK: SignedDuration = SignedDuration::from_hours(7 * 24);

fn at(text: &str) -> Timestamp {
    text.parse().expect("instant")
}

/// A prompt `prompt_id` in session `session`, sent now on the app's clock.
async fn prompt(api: &Api, prompt_id: &str, session: &str) {
    api.ingest(
        "claude-code",
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": prompt_id, "session_id": session, "prompt": "go", "cwd": "/r"}),
    )
    .await;
}

/// One prompt in the week before last week's end, two (in two sessions) in the last week; the
/// clock ends at 2026-11-18 12:00 UTC.
async fn two_weeks(clock: &ManualClock) -> Api {
    let api = harness().moving_clock(clock).start();
    clock.set(at("2026-11-08T10:00:00Z"));
    prompt(&api, "p1", "s1").await;
    clock.set(at("2026-11-12T10:00:00Z"));
    prompt(&api, "p2", "s2").await;
    clock.set(at("2026-11-13T10:00:00Z"));
    prompt(&api, "p3", "s3").await;
    clock.set(at("2026-11-18T12:00:00Z"));
    api
}

async fn stats(api: &Api, query: &[(&str, &str)]) -> Value {
    api.json_with("/api/stats", query).await
}

#[tokio::test]
async fn last_week_against_the_week_before() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    let body = stats(&api, &[("since", "2026-11-11T12:00:00Z"), ("previous", "true")]).await;
    let previous = &body["previous"];
    assert_eq!(previous["since"], "2026-11-04T12:00:00.000000+00:00");
    assert_eq!(previous["until"], "2026-11-11T12:00:00.000000+00:00");
    assert_eq!(previous["totals"]["prompts"], 1);
    assert_eq!(previous["changes"]["prompts"], json!({"now": 2.0, "before": 1.0, "change": 1.0}));
    assert_eq!(previous["changes"]["sessions"]["change"], 1.0);
    // Cost was 0 before: there is a "before" but no ratio to state.
    assert_eq!(previous["changes"]["cost_usd"], json!({"now": 0.0, "before": 0.0}));
}

#[tokio::test]
async fn without_an_end_the_period_ends_at_the_app_clock() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    let query = [("since", "2026-11-11T12:00:00Z"), ("previous", "true")];
    clock.advance(SignedDuration::from_hours(24));
    // Eight days now: the period before starts eight days before `since`.
    assert_eq!(stats(&api, &query).await["previous"]["since"], "2026-11-03T12:00:00.000000+00:00");
}

#[tokio::test]
async fn a_first_week_has_no_before_and_no_change() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    let previous =
        stats(&api, &[("since", "2026-11-07T00:00:00Z"), ("until", "2026-11-14T00:00:00Z"), ("previous", "true")]).await["previous"]
            .clone();
    assert_eq!(previous["totals"]["prompts"], 0);
    assert_eq!(previous["changes"]["prompts"], json!({"now": 3.0}), "no before, never +∞");
}

#[tokio::test]
async fn previous_is_only_there_when_asked_for_and_with_a_start() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    assert!(stats(&api, &[("since", "2026-11-11T12:00:00Z")]).await.get("previous").is_none());
    assert!(stats(&api, &[("previous", "true")]).await.get("previous").is_none(), "all time has no period before");
    let response = api.get("/api/stats").query(&[("previous", "maybe")]).send().await;
    assert_eq!(response.status, 422);
}

#[tokio::test]
async fn the_same_instant_in_two_time_zones_gives_the_same_period() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    let berlin = stats(&api, &[("since", "2026-11-11T13:00:00+01:00"), ("previous", "true")]).await;
    let new_york = stats(&api, &[("since", "2026-11-11T07:00:00-05:00"), ("previous", "true")]).await;
    assert_eq!(berlin["previous"], new_york["previous"]);
    assert_eq!(berlin["previous"]["until"], "2026-11-11T12:00:00.000000+00:00");
}

#[tokio::test]
async fn across_a_daylight_saving_change_the_period_before_is_as_long_and_touches_it() {
    let clock = ManualClock::new(at("2026-11-01T00:00:00Z"));
    let api = two_weeks(&clock).await;
    // Berlin leaves summer time on 25 October: this local week lasts 169 hours.
    let since = "2026-10-22T00:00:00+02:00";
    let until = "2026-10-29T00:00:00+01:00";
    let previous = stats(&api, &[("since", since), ("until", until), ("previous", "true")]).await["previous"].clone();
    let (start, end) = (at(previous["since"].as_str().unwrap()), at(previous["until"].as_str().unwrap()));
    assert_eq!(end, at(since), "no gap and no overlap");
    assert_eq!(end.duration_since(start), at(until).duration_since(at(since)));
    assert_eq!(end.duration_since(start), WEEK + SignedDuration::from_hours(1));
}
