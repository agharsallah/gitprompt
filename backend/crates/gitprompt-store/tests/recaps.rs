//! The weekly recaps already handled: durable, so a restart or a reindex never sends one twice.
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

use common::store;
use gitprompt_store::store::RecapRow;
use jiff::Timestamp;
use pretty_assertions::assert_eq;

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

#[test]
fn the_latest_recap_is_the_one_due_last_and_a_week_handled_again_is_replaced() {
    let store = store();
    assert_eq!(store.latest_recap().unwrap(), None);
    store.record_recap(at("2026-11-16T08:00:00Z"), Some("{\"body\":\"this week\"}")).unwrap();
    store.record_recap(at("2026-11-09T08:00:00Z"), None).unwrap();
    let latest = RecapRow { due: at("2026-11-16T08:00:00Z"), notification: Some("{\"body\":\"this week\"}".into()) };
    assert_eq!(store.latest_recap().unwrap(), Some(latest));
    store.record_recap(at("2026-11-16T08:00:00Z"), None).unwrap();
    assert_eq!(store.latest_recap().unwrap().unwrap().notification, None);
}

#[test]
fn a_recap_handled_survives_a_reindex_and_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let store = gitprompt_store::Store::open(&path).unwrap();
    store.record_recap(at("2026-11-16T08:00:00Z"), Some("{}")).unwrap();
    store.index().drop_all().unwrap();
    store.index().create().unwrap();
    drop(store);
    let reopened = gitprompt_store::Store::open(&path).unwrap();
    assert_eq!(reopened.latest_recap().unwrap().map(|r| r.due), Some(at("2026-11-16T08:00:00Z")));
}
