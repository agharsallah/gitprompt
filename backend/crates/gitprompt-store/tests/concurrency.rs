//! One store shared by threads (the daemon's ingest and dashboard reads), and all-or-nothing
//! transactions. New in the Rust port: the Python store had one connection.
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

use std::sync::atomic::{AtomicBool, Ordering};

use common::*;
use gitprompt_core::Error;
use gitprompt_store::stats::AggregateQuery;
use gitprompt_store::store::NewEvent;
use gitprompt_store::{PromptFilter, Store};
use pretty_assertions::assert_eq;
use serde_json::json;

const fn assert_send_sync<T: Send + Sync>() {}
const _: () = assert_send_sync::<Store>();

fn file_store(dir: &tempfile::TempDir) -> Store {
    Store::open(&dir.path().join("gitprompt.db")).unwrap()
}

#[test]
fn readers_never_fail_and_never_go_back_in_time_while_a_writer_ingests() {
    let dir = tempfile::tempdir().unwrap();
    let store = file_store(&dir);
    let writing = AtomicBool::new(true);

    let reads: Vec<Result<usize, String>> = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let (mut seen, mut reads) = (0, 0);
                    while writing.load(Ordering::Acquire) || reads == 0 {
                        let now = store.prompt_ids().map_err(|e| e.to_string())?.len();
                        store.search(&PromptFilter::default(), 20).map_err(|e| e.to_string())?;
                        store.aggregate(&AggregateQuery::default()).map_err(|e| e.to_string())?;
                        if now < seen {
                            return Err(format!("saw {now} prompts after {seen}"));
                        }
                        (seen, reads) = (now, reads + 1);
                    }
                    Ok(reads)
                })
            })
            .collect();
        for i in 0..300 {
            prompt(&format!("p{i}")).insert(&store);
        }
        writing.store(false, Ordering::Release);
        readers.into_iter().map(|r| r.join().unwrap()).collect()
    });

    assert!(reads.iter().all(Result::is_ok), "{reads:?}");
    assert_eq!(store.prompt_ids().unwrap().len(), 300);
}

#[test]
fn writes_from_several_threads_are_all_stored() {
    let dir = tempfile::tempdir().unwrap();
    let store = file_store(&dir);

    let errors: Vec<String> = std::thread::scope(|scope| {
        let writers: Vec<_> = (0..4)
            .map(|t| {
                let store = &store;
                scope.spawn(move || {
                    (0..50)
                        .filter_map(|i| {
                            let fields = obj(json!({"prompt_id": format!("t{t}-{i}"), "prompt": "x"}));
                            store.add_prompt("claude-code", &fields, &obj(json!({})), &json!({})).err().map(|e| e.to_string())
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        writers.into_iter().flat_map(|w| w.join().unwrap()).collect()
    });

    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(store.prompt_ids().unwrap().len(), 200);
}

#[test]
fn a_transaction_commits_its_writes_and_returns_the_closures_value() {
    let store = store();

    let id = store.transaction(|s| Ok(prompt("p1").insert_unindexed(s))).unwrap();

    assert_eq!(store.prompt_ids().unwrap(), vec![id]);
}

#[test]
fn a_transaction_that_fails_rolls_back_every_write() {
    let store = store();
    let kept = prompt("before").insert(&store);

    let result: gitprompt_core::Result<()> = store.transaction(|s| {
        prompt("p1").insert(s);
        event("Stop").insert(s);
        s.annotate(kept, &gitprompt_store::rows::Annotation { starred: true, ..Default::default() })?;
        s.add_events(&[NewEvent { source: "otel".into(), fields: obj(json!({"event": "x"})), raw: json!({}) }])?;
        s.index().set_version("v2")?;
        Err(Error::Invalid("abort".into()))
    });

    assert!(matches!(result, Err(Error::Invalid(_))));
    assert_eq!(store.prompt_ids().unwrap(), vec![kept]);
    assert_eq!(store.event_usage(None).unwrap(), vec![]);
    assert_eq!(store.annotation(kept).unwrap(), gitprompt_store::Annotation::default());
    assert_eq!(store.index().version().unwrap(), None);
    assert_eq!(store.count(&PromptFilter::default()).unwrap(), 1);
}

#[test]
fn a_store_can_be_shared_by_threads_through_a_reference() {
    let store = store();
    let id = prompt("p1").insert(&store);

    let seen = std::thread::scope(|scope| scope.spawn(|| store.get(id).unwrap().map(|p| p.row.id)).join().unwrap());

    assert_eq!(seen, Some(id));
}
