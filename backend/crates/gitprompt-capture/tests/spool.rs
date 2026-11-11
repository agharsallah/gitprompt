//! The offline queue: payloads queued while the daemon is down are drained into the pipeline at
//! start-up, in order, removing what was ingested and setting corrupted lines aside.
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

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use common::*;
use gitprompt_capture::spool::{Drained, Spool, Spooled, drain_into_pipeline};
use gitprompt_core::preferences::Preferences;
use gitprompt_core::{Error, Json};
use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::json;

fn spool(env: &TestEnv) -> Spool {
    Spool::new(env.path("gitprompt/spool"))
}

/// The payloads still queued, oldest first.
fn queued(spool: &Spool, source: &str) -> Vec<Json> {
    spool.pending(source).unwrap().iter().map(|l| Spooled::parse(l).expect("a queued payload").payload).collect()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn prompt(n: u32) -> Json {
    json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt_id": format!("p{n}"), "prompt": format!("task {n}")})
}

#[test]
fn the_spool_lives_in_the_gitprompt_home() {
    let env = TestEnv::new();
    assert_eq!(Spool::of(&env.settings), Spool::new(env.settings.spool_dir()));
}

#[test]
fn payloads_are_appended_one_compact_line_each_in_arrival_order_with_the_time_they_were_spooled() {
    let env = TestEnv::new();
    let spool = spool(&env).with_clock(Arc::new(|| "2026-11-07T10:11:12.5Z".parse().unwrap()));
    spool.append("claude-code", &prompt(1)).unwrap();
    spool
        .append(
            "claude-code",
            &json!({"hook_event_name": "Stop",
                                         "nested": {"multi": "line\ntext"}}),
        )
        .unwrap();

    let text = fs::read_to_string(env.path("gitprompt/spool/claude-code.jsonl")).unwrap();
    let lines: Vec<Json> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let spooled_at = "2026-11-07T10:11:12.500000+00:00";
    assert_eq!(
        lines,
        vec![
            json!({"spooled_at": spooled_at, "payload": prompt(1)}),
            json!({"spooled_at": spooled_at, "payload": {"hook_event_name": "Stop", "nested": {"multi": "line\ntext"}}})
        ]
    );
    assert_eq!(spool.pending("claude-code").unwrap().len(), 2);
}

#[test]
fn the_spool_is_private_to_the_user() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("claude-code", &prompt(1)).unwrap();
    let mode = |p: &std::path::Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&spool.dir), 0o700);
    assert_eq!(mode(&spool.queue_file("claude-code").unwrap()), 0o600);
}

#[test]
fn a_source_id_that_is_not_a_plain_name_is_refused() {
    let env = TestEnv::new();
    let err = spool(&env).append("../../etc/passwd", &prompt(1)).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)), "{err:?}");
    assert!(!env.path("etc").exists());
}

#[test]
fn sources_lists_every_non_empty_queue_sorted() {
    let env = TestEnv::new();
    let spool = spool(&env);
    assert_eq!(spool.sources().unwrap(), Vec::<String>::new(), "no directory yet");
    spool.append("cursor", &prompt(1)).unwrap();
    spool.append("claude-code", &prompt(1)).unwrap();
    write_text(&env.path("gitprompt/spool/codex.jsonl"), "");
    assert_eq!(spool.sources().unwrap(), vec!["claude-code", "cursor"]);
}

#[test]
fn draining_hands_every_payload_over_in_order_and_deletes_the_queue() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("claude-code", &prompt(1)).unwrap();
    spool.append("claude-code", &prompt(2)).unwrap();
    let seen = RefCell::new(vec![]);

    let drained = spool
        .drain("claude-code", &mut |p| {
            seen.borrow_mut().push(p);
            Ok(())
        })
        .unwrap();

    assert_eq!(drained, Drained { ingested: 2, corrupted: 0, remaining: 0 });
    assert_eq!(seen.into_inner(), vec![prompt(1), prompt(2)]);
    assert!(!spool.queue_file("claude-code").unwrap().exists());
}

#[test]
fn a_corrupted_line_is_set_aside_and_the_rest_is_drained() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let queue = spool.queue_file("claude-code").unwrap();
    write_text(&queue, &format!("{}\n{{\"half\": \n{}\n", prompt(1), prompt(2)));
    let seen = RefCell::new(vec![]);

    let drained = spool
        .drain("claude-code", &mut |p| {
            seen.borrow_mut().push(p);
            Ok(())
        })
        .unwrap();

    assert_eq!(drained, Drained { ingested: 2, corrupted: 1, remaining: 0 });
    assert_eq!(seen.into_inner(), vec![prompt(1), prompt(2)]);
    assert_eq!(fs::read_to_string(spool.corrupt_file("claude-code").unwrap()).unwrap(), "{\"half\": \n");
}

#[test]
fn a_failed_ingest_stops_the_drain_and_keeps_that_payload_and_the_later_ones_queued() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("claude-code", &prompt(1)).unwrap();
    spool.append("claude-code", &prompt(2)).unwrap();
    spool.append("claude-code", &prompt(3)).unwrap();

    let drained =
        spool.drain("claude-code", &mut |p| if p == prompt(2) { Err(Error::Storage("disk full".into())) } else { Ok(()) }).unwrap();

    assert_eq!(drained, Drained { ingested: 1, corrupted: 0, remaining: 2 });
    assert_eq!(queued(&spool, "claude-code"), vec![prompt(2), prompt(3)]);
    assert_eq!(spool.sources().unwrap(), vec!["claude-code"]);
}

#[test]
fn after_a_failed_drain_the_next_one_resumes_at_the_failed_payload_before_newer_ones() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("claude-code", &prompt(1)).unwrap();
    spool.append("claude-code", &prompt(2)).unwrap();
    spool.drain("claude-code", &mut |p| if p == prompt(2) { Err(Error::Storage("disk full".into())) } else { Ok(()) }).unwrap();
    spool.append("claude-code", &prompt(3)).unwrap();
    let seen = RefCell::new(vec![]);

    let drained = spool
        .drain("claude-code", &mut |p| {
            seen.borrow_mut().push(p);
            Ok(())
        })
        .unwrap();

    assert_eq!(drained, Drained { ingested: 2, corrupted: 0, remaining: 0 });
    assert_eq!(seen.into_inner(), vec![prompt(2), prompt(3)]);
    assert_eq!(spool.sources().unwrap(), Vec::<String>::new());
    let left: Vec<String> = fs::read_dir(&spool.dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, Vec::<String>::new(), "no queue, draining or offset file is left behind");
}

#[test]
fn a_payload_spooled_while_the_queue_drains_is_not_lost() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("claude-code", &prompt(1)).unwrap();
    let seen = RefCell::new(vec![]);

    spool
        .drain("claude-code", &mut |p| {
            if p == prompt(1) {
                spool.append("claude-code", &prompt(2)).unwrap(); // a hook fires while the daemon starts
            }
            seen.borrow_mut().push(p);
            Ok(())
        })
        .unwrap();

    let mut handled = seen.into_inner();
    handled.extend(queued(&spool, "claude-code"));
    assert_eq!(handled, vec![prompt(1), prompt(2)], "ingested now or still queued, in order");
}

#[test]
fn draining_an_empty_or_missing_queue_does_nothing() {
    let env = TestEnv::new();
    let drained = spool(&env).drain("claude-code", &mut |_| panic!("nothing to ingest")).unwrap();
    assert_eq!(drained, Drained::default());
}

// --- into the pipeline -----------------------------------------------------------------------

#[test]
fn start_up_drains_every_queue_into_the_store_in_order() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store();
    spool.append("claude-code", &prompt(1)).unwrap();
    spool.append("claude-code", &json!({"hook_event_name": "Stop", "session_id": "s1", "prompt_id": "p1"})).unwrap();
    spool.append("claude-code", &prompt(2)).unwrap();

    let (drained, captured) = drain_into_pipeline(&spool, &env.specs(), &store, &Preferences::default(), &blank()).unwrap();

    assert_eq!(drained.get("claude-code"), Some(&Drained { ingested: 3, corrupted: 0, remaining: 0 }));
    assert_eq!((captured.prompts, captured.events), (2, 1));
    let prompts: Vec<Option<String>> = all_prompts(&store).into_iter().map(|p| p.prompt).collect();
    assert_eq!(prompts, vec![Some("task 1".to_string()), Some("task 2".to_string())]);
    assert_eq!(all_events(&store)[0].prompt_id.as_deref(), Some("p1"));
    assert_eq!(spool.sources().unwrap(), Vec::<String>::new());
}

#[test]
fn drained_payloads_are_stored_as_received_when_they_were_spooled() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store();
    spool.append_at("claude-code", &prompt(1), at("2026-11-07T10:00:00Z")).unwrap();
    spool
        .append_at("claude-code", &json!({"hook_event_name": "Stop", "session_id": "s1", "prompt_id": "p1"}), at("2026-11-07T10:05:00Z"))
        .unwrap();

    drain_into_pipeline(&spool, &env.specs(), &store, &Preferences::default(), &blank()).unwrap();

    assert_eq!(all_prompts(&store)[0].received_at, "2026-11-07T10:00:00.000000+00:00");
    assert_eq!(all_events(&store)[0].received_at, "2026-11-07T10:05:00.000000+00:00");
}

#[test]
fn a_bare_payload_line_of_an_older_forwarder_is_stored_as_received_at_drain_time() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store().with_clock(Arc::new(|| "2026-11-08T09:00:00Z".parse().unwrap()));
    write_text(&spool.queue_file("claude-code").unwrap(), &format!("{}\n", prompt(1)));

    let (drained, captured) = drain_into_pipeline(&spool, &env.specs(), &store, &Preferences::default(), &blank()).unwrap();

    assert_eq!((drained["claude-code"].ingested, captured.prompts), (1, 1));
    assert_eq!(all_prompts(&store)[0].received_at, "2026-11-08T09:00:00.000000+00:00");
}

#[test]
fn a_prompt_both_delivered_and_spooled_is_stored_once() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store();
    ingest(&env.spec("claude-code"), &store, &prompt(1));
    spool.append("claude-code", &prompt(1)).unwrap();

    drain_into_pipeline(&spool, &env.specs(), &store, &Preferences::default(), &blank()).unwrap();

    assert_eq!(all_prompts(&store).len(), 1);
}

#[test]
fn a_queue_of_a_turned_off_source_is_drained_and_discarded() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store();
    spool.append("claude-code", &prompt(1)).unwrap();
    let prefs = Preferences { disabled_sources: vec!["claude-code".into()], ..Preferences::default() };

    let (drained, _) = drain_into_pipeline(&spool, &env.specs(), &store, &prefs, &blank()).unwrap();

    assert_eq!(drained["claude-code"].ingested, 1);
    assert_eq!(all_prompts(&store).len(), 0);
    assert_eq!(spool.sources().unwrap(), Vec::<String>::new());
}

#[test]
fn a_queue_of_a_source_without_a_spec_is_left_alone() {
    let env = TestEnv::new();
    let spool = spool(&env);
    spool.append("retired-tool", &prompt(1)).unwrap();

    let (drained, _) = drain_into_pipeline(&spool, &env.specs(), &store(), &Preferences::default(), &blank()).unwrap();

    assert_eq!(drained.get("retired-tool"), None);
    assert_eq!(spool.pending("retired-tool").unwrap().len(), 1);
}

#[test]
fn spooled_payloads_get_the_same_privacy_treatment_as_live_ones() {
    let env = TestEnv::new();
    let spool = spool(&env);
    let store = store();
    spool
        .append(
            "cursor",
            &json!({"hook_event_name": "beforeSubmitPrompt", "conversation_id": "c", "generation_id": "g",
                                   "user_email": "someone@example.test", "prompt": format!("use AKIA{}", "ABCDEFGHIJKLMNOP")}),
        )
        .unwrap();
    let prefs = Preferences { hide_secrets_on_ingest: true, ..Preferences::default() };

    drain_into_pipeline(&spool, &env.specs(), &store, &prefs, &blank()).unwrap();

    let [row] = all_prompts(&store).try_into().unwrap();
    assert_eq!(row.prompt.as_deref(), Some("use [hidden AWS access key]"));
    assert!(row.raw.get("user_email").is_none());
}
