//! The trigger-maintained event summary (`event_stats`) behind `event_usage(None)` and
//! `source_stats`: it must always equal what a full scan of the event table reports.

mod common;

use gitprompt_core::{Json, JsonMap};
use gitprompt_store::Store;
use serde_json::json;

fn fields(event: Option<&str>, prompt_id: &str) -> JsonMap {
    let mut map = JsonMap::new();
    if let Some(event) = event {
        map.insert("event".into(), json!(event));
    }
    map.insert("prompt_id".into(), json!(prompt_id));
    map
}

fn add(store: &Store, source: &str, event: Option<&str>, at: &str, size: usize) {
    let raw = json!({ "payload": "x".repeat(size) });
    store.add_event_at(source, &fields(event, "p1"), &raw, at).expect("add event");
}

/// The summary path and the scanning path (any `before` in the future scans every row) agree.
fn assert_exact(store: &Store) {
    let summary = store.event_usage(None).expect("summary");
    let scanned = store.event_usage(Some("9999-12-31")).expect("scan");
    assert_eq!(summary, scanned);
    let per_source = store.source_stats().expect("source stats");
    for usage in &scanned {
        assert!(per_source[&usage.source].events >= usage.rows);
    }
    let total: u64 = scanned.iter().map(|u| u.rows).sum();
    assert_eq!(per_source.values().map(|s| s.events).sum::<u64>(), total);
}

fn store_with_events() -> Store {
    let store = Store::open_in_memory().expect("store");
    add(&store, "claude-code", Some("Stop"), "2026-11-01T10:00:00.000000+00:00", 10);
    add(&store, "claude-code", Some("Stop"), "2026-11-02T10:00:00.000000+00:00", 30);
    add(&store, "claude-code", Some("PostToolUse"), "2026-11-02T11:00:00.000000+00:00", 500);
    add(&store, "claude-code", None, "2026-11-03T09:00:00.000000+00:00", 1);
    add(&store, "codex", Some("Stop"), "2026-11-03T10:00:00.000000+00:00", 7);
    store
}

#[test]
fn the_summary_counts_every_inserted_event_and_its_bytes() {
    let store = store_with_events();
    assert_exact(&store);
    let stop = store.event_usage(None).unwrap().into_iter().find(|u| u.source == "claude-code" && u.event.as_deref() == Some("Stop"));
    assert_eq!(stop.map(|u| u.rows), Some(2));
}

#[test]
fn events_without_a_name_are_their_own_group() {
    let store = store_with_events();
    let unnamed: Vec<_> = store.event_usage(None).unwrap().into_iter().filter(|u| u.event.is_none()).collect();
    assert_eq!(unnamed.len(), 1);
    assert_eq!(unnamed[0].rows, 1);
}

#[test]
fn the_summary_follows_deletions_by_kind_and_by_age() {
    let store = store_with_events();
    store.delete_events("claude-code", Some("Stop"), Some("2026-11-02T00:00:00.000000+00:00")).unwrap();
    assert_exact(&store);
    store.delete_events("claude-code", None, None).unwrap();
    assert_exact(&store);
    store.delete_events("codex", Some("Stop"), None).unwrap();
    assert_exact(&store);
    assert!(!store.source_stats().unwrap().contains_key("codex"), "a source with no rows left has no events");
}

#[test]
fn the_last_event_time_of_a_source_is_its_newest_event() {
    let store = store_with_events();
    assert_eq!(store.source_stats().unwrap()["codex"].last_at.as_deref(), Some("2026-11-03T10:00:00.000000+00:00"));
}

#[test]
fn the_summary_follows_rewrites_and_remaps() {
    let store = store_with_events();
    store
        .add_prompt_at(
            "claude-code",
            &fields(Some("UserPromptSubmit"), "p1"),
            &JsonMap::new(),
            &json!({}),
            "2026-11-01T09:00:00.000000+00:00",
        )
        .unwrap();
    let id = store.prompt_ids().unwrap()[0];
    store.rewrite(id, &|value: &Json| if value.is_object() { json!({"payload": "shorter"}) } else { value.clone() }).unwrap();
    assert_exact(&store);
    // A remap that renames every event of a source moves them all into one group.
    store
        .remap("claude-code", &|_raw: &Json| {
            let mut map = JsonMap::new();
            map.insert("event".into(), json!("Renamed"));
            map
        })
        .unwrap();
    assert_exact(&store);
    let groups: Vec<_> = store.event_usage(None).unwrap().into_iter().filter(|u| u.source == "claude-code").collect();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].event.as_deref(), Some("Renamed"));
}

#[test]
fn a_database_without_the_summary_is_backfilled_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    {
        let store = Store::open(&path).unwrap();
        add(&store, "claude-code", Some("Stop"), "2026-11-01T10:00:00.000000+00:00", 10);
        add(&store, "codex", Some("Stop"), "2026-11-01T11:00:00.000000+00:00", 20);
    }
    // As a database the Python daemon wrote: no summary table, no triggers.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "drop trigger event_stats_insert; drop trigger event_stats_delete; drop trigger event_stats_update; drop table event_stats;",
    )
    .unwrap();
    conn.execute(
        "insert into event (source, received_at, event, fields, raw) values ('codex', '2026-11-02T00:00:00+00:00', 'Stop', '{}', '{}')",
        [],
    )
    .unwrap();
    drop(conn);
    let store = Store::open(&path).unwrap();
    assert_exact(&store);
    assert_eq!(store.source_stats().unwrap()["codex"].events, 2);
}

#[test]
fn a_rebuild_repairs_a_summary_that_went_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let store = Store::open(&path).unwrap();
    add(&store, "claude-code", Some("Stop"), "2026-11-01T10:00:00.000000+00:00", 10);
    rusqlite::Connection::open(&path).unwrap().execute("update event_stats set rows = 99", []).unwrap();
    store.rebuild_event_stats().unwrap();
    assert_exact(&store);
}
