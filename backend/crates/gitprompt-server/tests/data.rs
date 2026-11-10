//! /api/data: where the database and backups are, and the clean-ups that always preview first.
//! Port of the data tests of test_settings.py (plus the retention pass through the Service).

mod common;

use common::{Api, api, harness, num};
use gitprompt_store::Store;
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Three stored events: one noise kind the spec drops, one api_request 40 days old, one recent Stop.
/// Planted with the store (ingest would drop the noise) and moved back in time with SQL.
fn plant_events(api: &Api, now: Timestamp) {
    let store = Store::open(&api.settings.db_path()).expect("store");
    let rows = [("claude-code-otel", "hook_registered", 0), ("claude-code-otel", "api_request", 40), ("claude-code", "Stop", 0)];
    let mut statements = String::new();
    for (source, event, days) in rows {
        let fields = json!({"event": event, "session_id": "s1"}).as_object().cloned().expect("object");
        let row = store.add_event(source, &fields, &json!({"x": event})).expect("event");
        let at = (now - SignedDuration::from_hours(24 * days)).strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00");
        statements += &format!("update event set received_at = '{at}' where id = {};", row.id);
    }
    api.sql(&statements);
}

async fn stored_events(api: &Api) -> f64 {
    api.json("/api/data").await["events"].as_array().expect("events").iter().map(|s| num(&s["rows"])).sum()
}

fn compact<'a>(api: &'a Api, query: &str, body: Value) -> common::Req<'a> {
    api.post(&format!("/api/data/compact{query}")).dashboard().json(body)
}

fn delete<'a>(api: &'a Api, query: &str, body: Value) -> common::Req<'a> {
    api.post(&format!("/api/data/delete-events{query}")).dashboard().json(body)
}

#[tokio::test]
async fn the_overview_names_the_database_its_events_per_source_and_the_backups() {
    let api = api();
    plant_events(&api, Timestamp::now());
    std::fs::create_dir_all(api.settings.backups()).expect("mkdir");
    std::fs::write(api.settings.backups().join("settings.json.20261107-101010"), "{}").expect("write");
    let body = api.json("/api/data").await;
    assert_eq!((&body["db_path"], &body["home"]), (&json!(api.settings.db_path().to_str()), &json!(api.settings.home.to_str())));
    assert!(num(&body["db_bytes"]) > 0.0);
    let per_source: Vec<(Value, Value)> =
        body["events"].as_array().expect("events").iter().map(|s| (s["source"].clone(), s["rows"].clone())).collect();
    assert_eq!(per_source.len(), 2);
    assert!(
        per_source.contains(&(json!("claude-code-otel"), json!(2))) && per_source.contains(&(json!("claude-code"), json!(1))),
        "{per_source:?}"
    );
    let backups: Vec<&Value> = body["backups"].as_array().expect("backups").iter().map(|b| &b["name"]).collect();
    assert_eq!(backups, [&json!("settings.json.20261107-101010")]);
    assert_eq!(body["receiver"], "http://127.0.0.1:7878");
}

#[tokio::test]
async fn compact_previews_by_default_and_changes_nothing() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let preview = compact(&api, "", json!({"older_than_days": 30})).send().await.json();
    assert_eq!((&preview["applied"], num(&preview["rows"])), (&json!(false), 2.0));
    assert_eq!(preview["size_after"], Value::Null);
    let mut why: Vec<(Value, Value)> =
        preview["removals"].as_array().expect("removals").iter().map(|r| (r["event"].clone(), r["why"].clone())).collect();
    why.sort_by_key(|(e, _)| e.to_string());
    assert_eq!(why, [(json!("api_request"), json!("expired")), (json!("hook_registered"), json!("dropped by spec"))]);
    assert_eq!(stored_events(&api).await, 3.0);
}

#[tokio::test]
async fn compact_applied_deletes_the_previewed_events() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let applied = compact(&api, "?dry_run=false", json!({"older_than_days": 30})).send().await.json();
    assert_eq!(applied["applied"], true);
    assert!(applied["size_after"].is_number());
    assert_eq!(stored_events(&api).await, 1.0);
}

#[tokio::test]
async fn compact_without_days_removes_only_what_the_specs_drop() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let preview = compact(&api, "", json!({})).send().await.json();
    assert_eq!(num(&preview["rows"]), 1.0);
}

#[tokio::test]
async fn compact_refuses_fewer_than_one_day() {
    let api = api();
    assert_eq!(compact(&api, "", json!({"older_than_days": 0})).send().await.status, 400);
}

#[tokio::test]
async fn deleting_every_event_is_not_offered() {
    let api = api();
    assert_eq!(delete(&api, "", json!({})).send().await.status, 400);
}

#[tokio::test]
async fn delete_events_before_a_date_previews_the_old_ones() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let cutoff = (Timestamp::now() - SignedDuration::from_hours(24 * 10)).strftime("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string();
    let preview = delete(&api, "", json!({"before": cutoff})).send().await.json();
    let removals: Vec<(Value, Value)> =
        preview["removals"].as_array().expect("removals").iter().map(|r| (r["source"].clone(), r["event"].clone())).collect();
    assert_eq!(removals, [(json!("claude-code-otel"), json!("api_request"))]);
}

#[tokio::test]
async fn delete_events_of_a_source_previews_then_deletes_with_the_confirmed_cutoff() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let preview = delete(&api, "", json!({"source": "claude-code"})).send().await.json();
    assert_eq!(num(&preview["rows"]), 1.0);
    assert_eq!(stored_events(&api).await, 3.0);
    let before = preview["removals"][0]["before"].clone(); // the dry run fixed "until now": confirm with the same time
    let applied = delete(&api, "?dry_run=false", json!({"source": "claude-code", "before": before})).send().await.json();
    assert_eq!(applied["applied"], true);
    assert_eq!(stored_events(&api).await, 2.0);
}

#[tokio::test]
async fn a_delete_dry_run_without_a_date_fixes_the_cutoff_at_the_app_clocks_now() {
    let now = Timestamp::from_second(Timestamp::now().as_second()).expect("now");
    let api = harness().clock(now).start();
    plant_events(&api, now - SignedDuration::from_mins(1));
    let preview = delete(&api, "", json!({"source": "claude-code"})).send().await.json();
    let before = preview["removals"][0]["before"].as_str().expect("before");
    assert_eq!(before.parse::<Timestamp>().expect("ISO-8601"), now);
}

#[tokio::test]
async fn retention_is_off_by_default_and_follows_the_preference_once_set() {
    let api = api();
    plant_events(&api, Timestamp::now());
    let service = api.service();
    assert_eq!(service.retention_pass().await.expect("pass"), 0);
    assert_eq!(stored_events(&api).await, 3.0);
    api.put("/api/settings").dashboard().json(json!({"event_retention_days": 30})).send().await;
    assert_eq!(service.retention_pass().await.expect("pass"), 1);
    assert_eq!(stored_events(&api).await, 2.0);
}
