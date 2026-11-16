//! Usage counts: how often each dashboard feature is used per day. Durable user data, so neither a
//! reindex nor a clean-up touches them.
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

use common::*;
use gitprompt_store::store::{MAX_USAGE_ACTIONS, UsageCount};
use pretty_assertions::assert_eq;

fn count(action: &str, day: &str, n: u64) -> UsageCount {
    UsageCount { action: action.into(), day: day.into(), n }
}

#[test]
fn increments_add_up_per_action_and_day() {
    let store = store();
    store.increment_usage("copy.prompt", "2026-11-03", 1).unwrap();
    store.increment_usage("copy.prompt", "2026-11-03", 2).unwrap();
    store.increment_usage("copy.prompt", "2026-11-04", 1).unwrap();
    store.increment_usage("page.opened.stats", "2026-11-03", 5).unwrap();

    assert_eq!(
        store.usage_between("2026-11-01", "2026-11-30").unwrap(),
        vec![count("copy.prompt", "2026-11-03", 3), count("page.opened.stats", "2026-11-03", 5), count("copy.prompt", "2026-11-04", 1)]
    );
}

#[test]
fn a_range_holds_both_its_end_days_and_nothing_else() {
    let store = store();
    for day in ["2026-11-02", "2026-11-03", "2026-11-04", "2026-11-05"] {
        store.increment_usage("export", day, 1).unwrap();
    }

    let days: Vec<String> = store.usage_between("2026-11-03", "2026-11-04").unwrap().into_iter().map(|c| c.day).collect();

    assert_eq!(days, ["2026-11-03", "2026-11-04"]);
}

#[test]
fn clearing_forgets_every_count() {
    let store = store();
    store.increment_usage("export", "2026-11-03", 1).unwrap();

    store.clear_usage().unwrap();

    assert_eq!(store.usage_between("2000-01-01", "2999-12-31").unwrap(), vec![]);
}

#[test]
fn counts_survive_a_reindex_and_are_kept_by_the_database_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let store = gitprompt_store::Store::open(&path).unwrap();
    prompt("p1").text("Fix it").insert(&store);
    store.increment_usage("reader.tab.events", "2026-11-03", 4).unwrap();

    // what `gitprompt reindex` does to the index and the event summary
    store.index().drop_all().unwrap();
    store.index().create().unwrap();
    store.rebuild_event_stats().unwrap();
    drop(store);

    let reopened = gitprompt_store::Store::open(&path).unwrap();
    assert_eq!(reopened.usage_between("2026-11-03", "2026-11-03").unwrap(), vec![count("reader.tab.events", "2026-11-03", 4)]);
}

#[test]
fn a_new_action_name_is_dropped_once_the_table_holds_the_most_it_keeps() {
    let store = store();
    for i in 0..MAX_USAGE_ACTIONS {
        store.increment_usage(&format!("a{i}"), "2026-11-03", 1).unwrap();
    }

    store.increment_usage("one.too.many", "2026-11-03", 1).unwrap();
    store.increment_usage("a0", "2026-11-04", 1).unwrap();

    let all = store.usage_between("2026-11-03", "2026-11-04").unwrap();
    assert_eq!(all.len(), MAX_USAGE_ACTIONS + 1);
    assert!(all.iter().all(|c| c.action != "one.too.many"));
    assert!(all.contains(&count("a0", "2026-11-04", 1)), "a kept action still counts");
}

#[test]
fn pruning_drops_the_days_before_the_cut_and_keeps_that_day() {
    let store = store();
    for day in ["2025-11-01", "2025-11-02", "2025-11-03"] {
        store.increment_usage("export", day, 1).unwrap();
    }

    assert_eq!(store.prune_usage("2025-11-02").unwrap(), 1);

    let days: Vec<String> = store.usage_between("2000-01-01", "2999-12-31").unwrap().into_iter().map(|c| c.day).collect();
    assert_eq!(days, ["2025-11-02", "2025-11-03"]);
}
