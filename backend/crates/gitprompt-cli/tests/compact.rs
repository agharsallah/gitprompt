//! `gitprompt compact`: a dry run by default (rows and MB per kind, nothing changes), `--apply`
//! deletes and VACUUMs. Port of cli.compact; events are planted with the store (ingest would drop
//! the noise kind).
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

use common::{Home, home, stdout};
use gitprompt_store::Store;
use pretty_assertions::assert_eq;
use serde_json::json;

/// One event the telemetry spec drops (noise) and one the hooks spec keeps.
fn plant(home: &Home) {
    let store = Store::open(&home.db_path()).expect("store");
    for (source, event) in [("claude-code-otel", "hook_registered"), ("claude-code", "Stop")] {
        let fields = json!({"event": event, "session_id": "s1"}).as_object().cloned().expect("object");
        store.add_event(source, &fields, &json!({"x": event})).expect("event");
    }
}

/// (source, event, rows) of the stored events, sorted.
fn stored(home: &Home) -> Vec<(String, Option<String>, u64)> {
    let store = Store::open(&home.db_path()).expect("store");
    let mut usage: Vec<_> = store.event_usage(None).expect("usage").into_iter().map(|u| (u.source, u.event, u.rows)).collect();
    usage.sort();
    usage
}

fn kept() -> Vec<(String, Option<String>, u64)> {
    vec![("claude-code".into(), Some("Stop".into()), 1)]
}

fn everything() -> Vec<(String, Option<String>, u64)> {
    vec![("claude-code".into(), Some("Stop".into()), 1), ("claude-code-otel".into(), Some("hook_registered".into()), 1)]
}

fn is_mb(text: &str) -> bool {
    text.strip_suffix(" MB").is_some_and(|n| n.replace(',', "").parse::<f64>().is_ok() && n.contains('.'))
}

#[test]
fn a_dry_run_lists_what_would_go_and_changes_nothing() {
    let home = home();
    plant(&home);
    let output = home.cmd(&["compact"]).assert().success().get_output().clone();
    let out = stdout(&output);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "{out}");
    let removal: Vec<&str> = lines[0].split("  ").map(str::trim).filter(|p| !p.is_empty()).collect();
    assert_eq!((removal[0], removal[2], removal[3]), ("1", "claude-code-otel hook_registered", "(dropped by spec)"), "{}", lines[0]);
    assert!(is_mb(removal[1]), "{}", lines[0]);
    assert!(lines[1].trim_start().starts_with("1  ") && lines[1].contains("events to remove, database is "), "{}", lines[1]);
    assert_eq!(lines[2], "Dry run: nothing changed. Run again with --apply to delete these events and VACUUM.");
    assert_eq!(stored(&home), everything());
}

#[test]
fn apply_deletes_the_listed_events_and_reports_the_new_size() {
    let home = home();
    plant(&home);
    let output = home.cmd(&["compact", "--apply"]).assert().success().get_output().clone();
    let out = stdout(&output);
    let last = out.lines().last().expect("a line");
    let size = last.strip_prefix("Done: database is now ").and_then(|s| s.strip_suffix('.')).expect("done line");
    assert!(is_mb(size), "{last}");
    assert_eq!(stored(&home), kept());
}

#[test]
fn expired_events_are_added_only_when_asked_and_fresh_ones_are_kept() {
    let home = home();
    plant(&home);
    let output = home.cmd(&["compact", "--apply", "--events-older-than", "30"]).assert().success().get_output().clone();
    assert!(!stdout(&output).contains("(expired)"), "{}", stdout(&output));
    assert_eq!(stored(&home), kept());
}

#[test]
fn nothing_to_remove_is_reported_as_zero() {
    let home = home();
    let output = home.cmd(&["compact"]).assert().success().get_output().clone();
    let out = stdout(&output);
    assert!(out.lines().next().is_some_and(|l| l.trim_start().starts_with("0  ") && l.contains("events to remove")), "{out}");
}
