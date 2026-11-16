//! /api/usage: the dashboard reports which features it used, per day, on this machine only. The day
//! comes from the app clock; a name that is not a short dotted lowercase one is refused.
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

use common::{api, harness};
use gitprompt_server::ManualClock;
use jiff::SignedDuration;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

async fn report(api: &common::Api, hits: Value) -> u16 {
    api.post("/api/usage").dashboard().json(json!({ "hits": hits })).send().await.status
}

#[tokio::test]
async fn a_report_is_answered_with_an_empty_204_and_counted_today() {
    let api = harness().clock("2026-01-05T09:00:00Z".parse().unwrap()).start();

    assert_eq!(report(&api, json!([{"action": "copy.prompt", "n": 2}, {"action": "export", "n": 1}])).await, 204);
    assert_eq!(report(&api, json!([{"action": "copy.prompt", "n": 1}])).await, 204);

    let body = api.json("/api/usage?days=1").await;
    assert_eq!(
        body,
        json!({"since": "2026-01-05", "until": "2026-01-05", "counts": [
            {"action": "copy.prompt", "day": "2026-01-05", "n": 3},
            {"action": "export", "day": "2026-01-05", "n": 1},
        ]})
    );
}

#[tokio::test]
async fn two_reports_either_side_of_midnight_land_on_two_days() {
    let clock = ManualClock::new("2026-01-05T23:59:30Z".parse().unwrap());
    let api = harness().moving_clock(&clock).start();

    report(&api, json!([{"action": "copy.prompt", "n": 1}])).await;
    clock.advance(SignedDuration::from_secs(60));
    report(&api, json!([{"action": "copy.prompt", "n": 1}])).await;

    let body = api.json("/api/usage?days=7").await;
    assert_eq!(
        body["counts"],
        json!([{"action": "copy.prompt", "day": "2026-01-05", "n": 1}, {"action": "copy.prompt", "day": "2026-01-06", "n": 1}])
    );
    assert_eq!((&body["since"], &body["until"]), (&json!("2025-12-31"), &json!("2026-01-06")));
    assert_eq!(api.json("/api/usage?days=1").await["counts"].as_array().unwrap().len(), 1, "one day is today alone");
}

#[tokio::test]
async fn days_default_to_a_week_and_must_be_a_sane_number() {
    let api = harness().clock("2026-01-10T00:00:00Z".parse().unwrap()).start();

    let body = api.json("/api/usage").await;
    assert_eq!((&body["since"], &body["until"]), (&json!("2026-01-04"), &json!("2026-01-10")));
    for bad in ["0", "367", "many"] {
        assert_eq!(api.get("/api/usage").query(&[("days", bad)]).send().await.status, 422, "days={bad}");
    }
}

#[tokio::test]
async fn a_name_or_count_that_does_not_validate_is_refused_and_nothing_is_stored() {
    let api = api();
    let long = "a".repeat(49);
    let refused = [
        json!([{"action": "Copy", "n": 1}]),
        json!([{"action": "has space", "n": 1}]),
        json!([{"action": long, "n": 1}]),
        json!([{"action": "ok", "n": 0}]),
        json!([{"action": "ok", "n": 1001}]),
        json!([{"action": "fine", "n": 1}, {"action": "no/slash", "n": 1}]),
        json!((0..65).map(|i| json!({"action": format!("a{i}"), "n": 1})).collect::<Vec<_>>()),
    ];
    for hits in refused {
        assert_eq!(report(&api, hits).await, 422);
    }
    assert_eq!(api.json("/api/usage?days=30").await["counts"], json!([]));
}

#[tokio::test]
async fn writes_need_the_dashboard_header_and_a_json_body() {
    let api = api();
    let body = json!({"hits": [{"action": "export", "n": 1}]});
    assert_eq!(api.post("/api/usage").json(body).send().await.status, 403);
    assert_eq!(api.delete("/api/usage").send().await.status, 403);
    assert_eq!(api.post("/api/usage").dashboard().send().await.status, 422);
}

#[tokio::test]
async fn clearing_forgets_every_count() {
    let api = api();
    report(&api, json!([{"action": "export", "n": 1}])).await;

    assert_eq!(api.delete("/api/usage").dashboard().send().await.status, 204);

    assert_eq!(api.json("/api/usage?days=30").await["counts"], json!([]));
}

#[tokio::test]
async fn a_day_is_dropped_once_no_window_reaches_it() {
    let clock = ManualClock::new("2026-01-05T12:00:00Z".parse().unwrap());
    let api = harness().moving_clock(&clock).start();
    report(&api, json!([{"action": "copy.prompt", "n": 1}])).await;

    // 365 days later 2026-01-05 is still the first day of the longest window; one more and it is not.
    clock.advance(SignedDuration::from_hours(24 * 365));
    report(&api, json!([{"action": "export", "n": 1}])).await;
    let kept = api.json("/api/usage?days=366").await;
    assert_eq!(kept["since"], json!("2026-01-05"));
    assert_eq!(kept["counts"].as_array().unwrap().len(), 2);

    clock.advance(SignedDuration::from_hours(24));
    report(&api, json!([{"action": "export", "n": 1}])).await;
    let counts = api.json("/api/usage?days=366").await["counts"].clone();
    assert_eq!(counts, json!([{"action": "export", "day": "2027-01-05", "n": 1}, {"action": "export", "day": "2027-01-06", "n": 1}]));
}
