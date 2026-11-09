//! Storage hygiene: event usage and deletion, noise/expired/selected, plan/apply, compact and the
//! retention pass. Port of capture/compact.py, capture/retention.py, tests/capture/test_compact.py
//! and the data parts of tests/api/test_settings.py.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use common::*;
use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::spec::Spec;
use gitprompt_core::{Env, Error};
use gitprompt_store::Store;
use gitprompt_store::housekeeping::{self, Removal, Report, Why};
use gitprompt_store::rows::Annotation;
use gitprompt_store::store::EventUsage;
use pretty_assertions::assert_eq;
use serde_json::json;

const OTEL: &str = "claude-code-otel";
const NOW: &str = "2026-11-09T12:00:00.000000+00:00";

fn now() -> Timestamp {
    parse(NOW)
}

fn days_ago(days: i64) -> String {
    ts(now() - SignedDuration::from_hours(24 * days))
}

/// A spec that keeps only `accept` (when non-empty) and drops events matching `drop`.
fn spec(id: &str, accept: &[&str], drop: &[&str]) -> Arc<Spec> {
    let drop: serde_json::Map<String, serde_json::Value> = drop.iter().map(|p| (p.to_string(), json!("noise"))).collect();
    let spec = json!({
        "id": id, "name": id, "detect": [], "install": {"file": "x", "merge": {}},
        "accept_events": accept, "drop_events": drop,
    });
    Arc::new(serde_json::from_value(spec).expect("a valid spec"))
}

fn specs() -> BTreeMap<String, Arc<Spec>> {
    BTreeMap::from([(OTEL.to_string(), spec(OTEL, &[], &["hook_*"])), ("claude-code".to_string(), spec("claude-code", &[], &[]))])
}

fn add(store: &Store, source: &str, name: &str, days: i64) {
    event(name).source(source).raw(json!({"event.name": name})).at(&days_ago(days)).insert(store);
}

/// Stored event names, oldest first.
fn stored(store: &Store) -> Vec<String> {
    store.events_for_session("s1").unwrap().into_iter().map(|e| e.event.unwrap_or_default()).collect()
}

fn kinds(removals: &[Removal]) -> Vec<(String, Option<String>, u64, Why)> {
    let mut kinds: Vec<_> = removals.iter().map(|r| (r.source.clone(), r.event.clone(), r.rows, r.why)).collect();
    kinds.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    kinds
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

// --- cutoff -------------------------------------------------------------------------------------

#[test]
fn the_cutoff_is_now_minus_the_days_as_a_received_at() {
    assert_eq!(housekeeping::cutoff(30, now()).unwrap(), "2026-10-10T12:00:00.000000+00:00");
}

#[test]
fn retention_needs_at_least_one_day() {
    assert!(matches!(housekeeping::cutoff(0, now()), Err(Error::Invalid(_))));
    assert!(matches!(housekeeping::cutoff(-1, now()), Err(Error::Invalid(_))));
    assert!(matches!(housekeeping::expired(&store(), 0, now(), &[]), Err(Error::Invalid(_))));
}

// --- event usage and deletion -------------------------------------------------------------------

#[test]
fn event_usage_counts_rows_and_bytes_per_source_and_event_largest_first() {
    let store = store();
    event("small").raw(json!({})).insert(&store);
    event("big").raw(json!({"blob": "x".repeat(5000)})).insert(&store);
    event("big").raw(json!({"blob": "x".repeat(5000)})).insert(&store);
    event("x").unnamed().source("other").raw(json!({"blob": "y".repeat(1000)})).insert(&store);

    let usage = store.event_usage(None).unwrap();

    let shape: Vec<_> = usage.iter().map(|u| (u.source.as_str(), u.event.as_deref(), u.rows)).collect();
    assert_eq!(shape, vec![("claude-code", Some("big"), 2), ("other", None, 1), ("claude-code", Some("small"), 1)]);
    assert!(usage[0].bytes > 10_000 && usage[2].bytes > 0, "{usage:?}");
}

#[test]
fn event_usage_before_a_time_counts_only_older_rows() {
    let store = store();
    add(&store, "claude-code", "Stop", 40);
    add(&store, "claude-code", "Stop", 1);

    let usage = store.event_usage(Some(&days_ago(30))).unwrap();

    assert_eq!(usage.iter().map(|u| (u.event.clone(), u.rows)).collect::<Vec<_>>(), vec![(some("Stop"), 1)]);
}

#[test]
fn delete_events_removes_one_kind_of_one_source() {
    let store = store();
    add(&store, "claude-code", "Stop", 0);
    add(&store, "claude-code", "Stop", 0);
    add(&store, "claude-code", "PostToolUse", 0);
    add(&store, "codex", "Stop", 0);

    assert_eq!(store.delete_events("claude-code", Some("Stop"), None).unwrap(), 2);
    assert_eq!(stored(&store), vec!["PostToolUse", "Stop"]);
}

#[test]
fn deleting_events_without_a_name_matches_only_unnamed_events() {
    let store = store();
    event("x").unnamed().insert(&store);
    add(&store, "claude-code", "Stop", 0);

    assert_eq!(store.delete_events("claude-code", None, None).unwrap(), 1);
    assert_eq!(stored(&store), vec!["Stop"]);
}

#[test]
fn delete_events_before_a_time_keeps_newer_rows() {
    let store = store();
    add(&store, "claude-code", "Stop", 40);
    add(&store, "claude-code", "Stop", 1);

    assert_eq!(store.delete_events("claude-code", Some("Stop"), Some(&days_ago(30))).unwrap(), 1);
    assert_eq!(store.event_usage(None).unwrap().iter().map(|u| u.rows).sum::<u64>(), 1);
}

#[test]
fn deleting_events_never_touches_prompts_or_annotations() {
    let store = store();
    let id = prompt("p1").at("2000-01-01T00:00:00.000000+00:00").insert(&store);
    let note = Annotation { starred: true, ..Default::default() };
    store.annotate(id, &note).unwrap();
    add(&store, "claude-code", "UserPromptSubmit", 40);

    store.delete_events("claude-code", Some("UserPromptSubmit"), None).unwrap();

    assert_eq!(store.prompt_ids().unwrap(), vec![id]);
    assert_eq!(store.annotation(id).unwrap(), note);
}

// --- noise / expired / selected -----------------------------------------------------------------

#[test]
fn noise_lists_stored_events_the_spec_no_longer_keeps() {
    let store = store();
    for name in ["hook_registered", "hook_registered", "hook_execution_start", "api_request"] {
        add(&store, OTEL, name, 0);
    }
    add(&store, "unknown-source", "hook_registered", 0);

    let removals = housekeeping::noise(&store, &specs()).unwrap();

    assert_eq!(
        kinds(&removals),
        vec![
            (OTEL.into(), some("hook_execution_start"), 1, Why::DroppedBySpec),
            (OTEL.into(), some("hook_registered"), 2, Why::DroppedBySpec),
        ]
    );
    assert!(removals.iter().all(|r| r.bytes > 0 && r.before.is_none()), "{removals:?}");
}

#[test]
fn noise_drops_unnamed_events_of_a_spec_with_accept_events() {
    let store = store();
    event("x").unnamed().source("strict").insert(&store);
    add(&store, "strict", "kept", 0);
    let specs = BTreeMap::from([("strict".to_string(), spec("strict", &["kept"], &[]))]);

    let removals = housekeeping::noise(&store, &specs).unwrap();

    assert_eq!(kinds(&removals), vec![("strict".into(), None, 1, Why::DroppedBySpec)]);
}

#[test]
fn expired_lists_events_older_than_the_days_except_skipped_kinds() {
    let store = store();
    add(&store, "claude-code", "Stop", 40);
    add(&store, "claude-code", "Stop", 2);
    add(&store, OTEL, "hook_registered", 40);
    let skip =
        vec![Removal { source: OTEL.into(), event: some("hook_registered"), rows: 1, bytes: 1, why: Why::DroppedBySpec, before: None }];

    let removals = housekeeping::expired(&store, 30, now(), &skip).unwrap();

    assert_eq!(kinds(&removals), vec![("claude-code".into(), some("Stop"), 1, Why::Expired)]);
    assert_eq!(removals[0].before, Some(housekeeping::cutoff(30, now()).unwrap()));
}

#[test]
fn an_event_received_exactly_at_the_cutoff_has_not_expired() {
    let store = store();
    let cutoff = now() - SignedDuration::from_hours(24 * 30);
    event("on-cutoff").at(&ts(cutoff)).insert(&store);
    event("just-older").at(&ts(cutoff - SignedDuration::from_micros(1))).insert(&store);

    let removals = housekeeping::expired(&store, 30, now(), &[]).unwrap();

    assert_eq!(kinds(&removals), vec![("claude-code".into(), some("just-older"), 1, Why::Expired)]);
}

#[test]
fn selected_lists_events_of_a_source_received_before_a_time() {
    let store = store();
    add(&store, "claude-code", "Stop", 40);
    add(&store, "claude-code", "Stop", 1);
    add(&store, OTEL, "api_request", 40);
    let before = days_ago(10);

    let both = housekeeping::selected(&store, Some("claude-code"), Some(&before)).unwrap();
    let by_source = housekeeping::selected(&store, Some("claude-code"), None).unwrap();
    let by_time = housekeeping::selected(&store, None, Some(&before)).unwrap();

    assert_eq!(kinds(&both), vec![("claude-code".into(), some("Stop"), 1, Why::Selected)]);
    assert_eq!(both[0].before, Some(before));
    assert_eq!(kinds(&by_source), vec![("claude-code".into(), some("Stop"), 2, Why::Selected)]);
    assert_eq!(kinds(&by_time).len(), 2);
}

#[test]
fn selecting_every_source_at_any_time_lists_every_event_kind() {
    let store = store();
    add(&store, "claude-code", "Stop", 0);
    add(&store, OTEL, "api_request", 0);

    let all = housekeeping::selected(&store, None, None).unwrap();

    assert_eq!(all.iter().map(|r| r.rows).sum::<u64>(), 2);
    assert!(all.iter().all(|r| r.before.is_none()));
}

// --- remove / plan ------------------------------------------------------------------------------

#[test]
fn removing_events_marks_the_index_stale() {
    let store = store();
    store.index().set_version("v1").unwrap();
    add(&store, "claude-code", "Stop", 40);

    let removed = housekeeping::remove(&store, &housekeeping::expired(&store, 30, now(), &[]).unwrap()).unwrap();

    assert_eq!(removed, 1);
    assert_eq!(store.index().version().unwrap(), None);
}

#[test]
fn removing_nothing_keeps_the_index() {
    let store = store();
    store.index().set_version("v1").unwrap();

    assert_eq!(housekeeping::remove(&store, &[]).unwrap(), 0);
    assert_eq!(store.index().version().unwrap().as_deref(), Some("v1"));
}

#[test]
fn a_report_totals_the_rows_and_bytes_of_its_removals() {
    let removal = |rows, bytes| Removal { source: "s".into(), event: None, rows, bytes, why: Why::Selected, before: None };
    let report = Report { removals: vec![removal(2, 100), removal(3, 50)], size_before: 0, size_after: None };

    assert_eq!((report.rows(), report.bytes()), (5, 150));
}

#[test]
fn a_plan_changes_nothing_unless_applied() {
    let store = store();
    add(&store, "claude-code", "Stop", 0);
    let removals = housekeeping::selected(&store, None, None).unwrap();

    let report = housekeeping::plan(&store, removals.clone(), false).unwrap();

    assert_eq!(report, Report { removals, size_before: store.size().unwrap(), size_after: None });
    assert_eq!(stored(&store), vec!["Stop"]);
}

#[test]
fn an_applied_plan_deletes_and_reports_the_size_after() {
    let store = store();
    add(&store, "claude-code", "Stop", 0);
    let removals = housekeeping::selected(&store, None, None).unwrap();

    let report = housekeeping::plan(&store, removals, true).unwrap();

    assert_eq!(report.size_after, Some(store.size().unwrap()));
    assert_eq!(stored(&store), Vec::<String>::new());
}

// --- compact ------------------------------------------------------------------------------------

#[test]
fn compact_is_a_dry_run_unless_applied() {
    let store = store();
    for name in ["hook_registered", "hook_registered", "hook_execution_start", "api_request"] {
        add(&store, OTEL, name, 0);
    }
    add(&store, "unknown-source", "hook_registered", 0);

    let plan = housekeeping::compact(&store, &specs(), None, now(), false).unwrap();
    assert_eq!((plan.rows(), plan.size_after), (3, None));
    assert!(plan.bytes() > 0);
    assert_eq!(stored(&store).len(), 5);

    let done = housekeeping::compact(&store, &specs(), None, now(), true).unwrap();
    assert_eq!(done.rows(), 3);
    assert!(done.size_after.is_some());
    assert_eq!(stored(&store), vec!["api_request", "hook_registered"]);
    assert_eq!(housekeeping::compact(&store, &specs(), None, now(), false).unwrap().removals, vec![]);
}

#[test]
fn compact_with_retention_counts_old_noise_once_and_never_removes_prompts_or_annotations() {
    let store = store();
    let id = prompt("p1").at("2000-01-01T00:00:00.000000+00:00").insert(&store);
    let note = Annotation { starred: true, ..Default::default() };
    store.annotate(id, &note).unwrap();
    add(&store, "claude-code", "Stop", 40);
    add(&store, OTEL, "hook_registered", 40);
    add(&store, "claude-code", "PostToolUse", 2);

    let plan = housekeeping::compact(&store, &specs(), Some(30), now(), false).unwrap();
    assert_eq!(
        kinds(&plan.removals),
        vec![("claude-code".into(), some("Stop"), 1, Why::Expired), (OTEL.into(), some("hook_registered"), 1, Why::DroppedBySpec)]
    );

    housekeeping::compact(&store, &specs(), Some(30), now(), true).unwrap();
    assert_eq!(stored(&store), vec!["PostToolUse"]);
    assert_eq!(store.prompt_ids().unwrap(), vec![id]);
    assert_eq!(store.get(id).unwrap().unwrap().annotation, note);
}

#[test]
fn compact_with_less_than_one_day_is_invalid() {
    assert!(matches!(housekeeping::compact(&store(), &specs(), Some(0), now(), false), Err(Error::Invalid(_))));
}

// --- size and vacuum ----------------------------------------------------------------------------

#[test]
fn vacuum_returns_the_space_of_deleted_events() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("gitprompt.db")).unwrap();
    store
        .transaction(|s| {
            (0..200).for_each(|_| {
                event("big").raw(json!({"blob": "x".repeat(4000)})).insert(s);
            });
            Ok(())
        })
        .unwrap();
    let full = store.size().unwrap();

    store.delete_events("claude-code", Some("big"), None).unwrap();
    store.vacuum().unwrap();

    assert!(store.size().unwrap() < full / 4, "{} not much smaller than {full}", store.size().unwrap());
}

#[test]
fn vacuum_keeps_every_stored_row() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("gitprompt.db")).unwrap();
    let id = prompt("p1").insert(&store);
    let kept = event("Stop").insert(&store);

    store.vacuum().unwrap();

    assert_eq!(store.prompt_ids().unwrap(), vec![id]);
    assert_eq!(store.events_for_session("s1").unwrap(), vec![kept]);
}

// --- retention pass -----------------------------------------------------------------------------

fn preferences(home: &Path, json: &str) -> PreferenceFile {
    std::fs::write(home.join(gitprompt_core::preferences::FILE_NAME), json).unwrap();
    PreferenceFile::new(home, Env::default())
}

/// test_settings.py `events`: one noise kind, one old and one recent event.
fn retention_store() -> Store {
    let store = store();
    add(&store, OTEL, "hook_registered", 0);
    add(&store, OTEL, "api_request", 40);
    add(&store, "claude-code", "Stop", 2);
    store
}

#[test]
fn retention_is_off_by_default() {
    let home = tempfile::tempdir().unwrap();
    let store = retention_store();

    let removed = housekeeping::retention_pass(&store, &PreferenceFile::new(home.path(), Env::default()), now()).unwrap();

    assert_eq!((removed, stored(&store).len()), (0, 3));
}

#[test]
fn a_retention_pass_deletes_events_older_than_the_preference() {
    let home = tempfile::tempdir().unwrap();
    let store = retention_store();
    store.index().set_version("v1").unwrap();

    let removed = housekeeping::retention_pass(&store, &preferences(home.path(), r#"{"event_retention_days": 30}"#), now()).unwrap();

    assert_eq!(removed, 1);
    assert_eq!(stored(&store), vec!["hook_registered", "Stop"]);
    assert_eq!(store.index().version().unwrap(), None);
}

#[test]
fn a_changed_preference_applies_on_the_next_pass_without_a_restart() {
    let home = tempfile::tempdir().unwrap();
    let store = retention_store();
    let prefs = preferences(home.path(), r#"{"event_retention_days": 30}"#);
    housekeeping::retention_pass(&store, &prefs, now()).unwrap();

    std::fs::write(&prefs.path, r#"{"event_retention_days": 1}"#).unwrap();

    assert_eq!(housekeeping::retention_pass(&store, &prefs, now()).unwrap(), 1);
    assert_eq!(stored(&store), vec!["hook_registered"]);
}

#[test]
fn an_invalid_preferences_file_deletes_nothing() {
    let home = tempfile::tempdir().unwrap();
    let store = retention_store();

    let removed = housekeeping::retention_pass(&store, &preferences(home.path(), "{oops"), now()).unwrap();

    assert_eq!((removed, stored(&store).len()), (0, 3));
}

#[test]
fn event_usage_rows_are_what_selected_reports() {
    let store = retention_store();
    let usage: Vec<EventUsage> = store.event_usage(None).unwrap();
    let selected = housekeeping::selected(&store, None, None).unwrap();

    let pairs = |rows: Vec<(String, Option<String>, u64, u64)>| {
        let mut rows = rows;
        rows.sort();
        rows
    };
    assert_eq!(
        pairs(usage.into_iter().map(|u| (u.source, u.event, u.rows, u.bytes)).collect()),
        pairs(selected.into_iter().map(|r| (r.source, r.event, r.rows, r.bytes)).collect())
    );
}
