//! Prompts and events: writes from the receiver, dedup, the session join, reads, annotations and
//! edits. Port of the storage-level assertions of storage/store.py and its API tests.
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

use std::collections::BTreeMap;

use common::*;
use gitprompt_core::{Error, Json};
use gitprompt_store::rows::Annotation;
use gitprompt_store::store::{CHUNK, HourCount, NewEvent, Sample, SourceStats};
use gitprompt_store::{PromptFilter, Store};
use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::json;

fn hook_payload() -> Json {
    json!({
        "session_id": "abc123",
        "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
        "cwd": "/work/app",
        "hook_event_name": "UserPromptSubmit",
        "prompt": "Write a function to calculate the factorial of a number",
    })
}

// --- add_prompt ---------------------------------------------------------------------------------

#[test]
fn a_stored_prompt_keeps_its_mapped_text_fields_context_and_raw_payload() {
    let store = store();
    let fields = obj(hook_payload());
    let context = obj(json!({"git_head": "abc", "tool_version": "2.1.316"}));
    let raw = hook_payload();

    let row = store.add_prompt("claude-code", &fields, &context, &raw).unwrap().unwrap();

    assert_eq!(row.source, "claude-code");
    assert_eq!(row.prompt.as_deref(), Some("Write a function to calculate the factorial of a number"));
    assert_eq!((&row.fields, &row.context, &row.raw), (&fields, &context, &raw));
    assert_eq!(store.prompts(&[row.id]).unwrap(), vec![row]);
}

#[test]
fn a_prompt_whose_mapped_prompt_is_not_a_string_has_no_text() {
    let store = store();
    let numeric = store.add_prompt("x", &obj(json!({"prompt": 42, "prompt_id": "a"})), &obj(json!({})), &json!({})).unwrap();
    let missing = store.add_prompt("x", &obj(json!({"prompt_id": "b"})), &obj(json!({})), &json!({})).unwrap();

    assert_eq!(numeric.unwrap().prompt, None);
    assert_eq!(missing.unwrap().prompt, None);
}

#[test]
fn received_at_is_the_current_utc_time_with_microseconds() {
    let store = store();
    let before = Timestamp::now();
    let row = store.add_prompt("x", &obj(json!({})), &obj(json!({})), &json!({})).unwrap().unwrap();
    let after = Timestamp::now();

    let at = parse(&row.received_at);
    assert!(before <= at && at <= after, "{} not between {before} and {after}", row.received_at);
    assert_eq!(row.received_at, ts(at), "UTC ISO-8601 with microseconds and +00:00");
}

#[test]
fn add_prompt_at_stores_the_given_received_at() {
    let store = store();
    let row = store
        .add_prompt_at("x", &obj(json!({"prompt": "hi"})), &obj(json!({})), &json!({}), "2026-01-02T03:04:05.000006+00:00")
        .unwrap()
        .unwrap();

    assert_eq!(store.prompts(&[row.id]).unwrap()[0].received_at, "2026-01-02T03:04:05.000006+00:00");
}

#[test]
fn the_same_prompt_id_from_the_same_source_is_stored_once() {
    let store = store();
    let fields = obj(hook_payload());
    let first = store.add_prompt("claude-code", &fields, &obj(json!({})), &hook_payload()).unwrap();
    let again = store.add_prompt("claude-code", &fields, &obj(json!({})), &hook_payload()).unwrap();

    assert!(first.is_some());
    assert_eq!(again, None);
    assert_eq!(store.prompt_ids().unwrap(), vec![first.unwrap().id]);
}

#[test]
fn the_same_prompt_id_from_another_source_is_a_new_prompt() {
    let store = store();
    let a = prompt("p1").source("claude-code").insert_unindexed(&store);
    let b = prompt("p1").source("codex").insert_unindexed(&store);

    assert_eq!(store.prompt_ids().unwrap(), vec![a, b]);
}

#[test]
fn prompts_without_a_prompt_id_never_collide() {
    let store = store();
    let a = PromptBuilder::new().source("cursor").insert_unindexed(&store);
    let b = PromptBuilder::new().source("cursor").insert_unindexed(&store);

    assert_eq!(store.prompt_ids().unwrap(), vec![a, b]);
}

// --- add_event / add_events ---------------------------------------------------------------------

#[test]
fn an_event_gets_its_event_session_and_prompt_id_columns_from_its_fields() {
    let store = store();
    let fields = obj(json!({"event": "Stop", "session_id": "s1", "prompt_id": "p1", "extra": 1}));
    let raw = json!({"hook_event_name": "Stop", "x": [1, 2]});

    let row = store.add_event("claude-code", &fields, &raw).unwrap();

    assert_eq!(
        (row.source.as_str(), row.event.as_deref(), row.session_id.as_deref(), row.prompt_id.as_deref()),
        ("claude-code", Some("Stop"), Some("s1"), Some("p1"))
    );
    assert_eq!((&row.fields, &row.raw), (&fields, &raw));
    assert_eq!(store.events_for_prompts(&["p1".into()]).unwrap(), vec![row]);
}

#[test]
fn an_event_without_those_fields_has_empty_columns() {
    let store = store();
    let row = store.add_event("x", &obj(json!({})), &json!({})).unwrap();

    assert_eq!((row.event, row.session_id, row.prompt_id), (None, None, None));
}

#[test]
fn a_batch_of_events_is_stored_in_order() {
    let store = store();
    let batch: Vec<NewEvent> = ["a", "b", "c"]
        .iter()
        .map(|name| NewEvent { source: "otel".into(), fields: obj(json!({"event": name, "session_id": "s"})), raw: json!({"n": name}) })
        .collect();

    let rows = store.add_events(&batch).unwrap();

    let names: Vec<_> = rows.iter().map(|r| r.event.clone().unwrap()).collect();
    assert_eq!(names, ["a", "b", "c"]);
    assert!(rows[0].id < rows[1].id && rows[1].id < rows[2].id);
    assert_eq!(store.events_for_session("s").unwrap(), rows);
}

#[test]
fn an_empty_batch_stores_nothing() {
    let store = store();
    assert_eq!(store.add_events(&[]).unwrap(), vec![]);
    assert_eq!(store.event_usage(None).unwrap(), vec![]);
}

#[test]
fn a_batch_with_one_failing_row_stores_none_of_its_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let store = Store::open(&path).unwrap();
    // Failure injected at the database: a trigger refusing one event name.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("create trigger refuse before insert on event when new.event = 'boom' begin select raise(abort, 'refused'); end;")
        .unwrap();
    let batch: Vec<NewEvent> = ["ok", "boom", "ok"]
        .iter()
        .map(|name| NewEvent { source: "otel".into(), fields: obj(json!({"event": name})), raw: json!({}) })
        .collect();

    assert!(store.add_events(&batch).is_err());
    assert_eq!(store.event_usage(None).unwrap(), vec![]);
}

// --- session join -------------------------------------------------------------------------------

#[test]
fn a_turn_name_is_the_source_and_row_id() {
    assert_eq!(Store::turn_name("cursor", 7), "cursor#7");
}

#[test]
fn naming_a_turn_stores_the_name_as_its_prompt_id() {
    let store = store();
    let id = PromptBuilder::new().source("cursor").insert_unindexed(&store);

    let name = store.name_turn(id, "cursor").unwrap();

    assert_eq!(name, format!("cursor#{id}"));
    assert_eq!(store.prompts(&[id]).unwrap()[0].fields["prompt_id"], json!(name));
    assert_eq!(store.prompts_of_turns(&[name]).unwrap(), vec![id]);
}

#[test]
fn the_latest_turn_is_the_prompt_id_of_the_newest_prompt_of_that_source_and_session() {
    let store = store();
    prompt("old").source("cursor").session("s1").insert_unindexed(&store);
    prompt("new").source("cursor").session("s1").insert_unindexed(&store);
    prompt("other-session").source("cursor").session("s2").insert_unindexed(&store);
    prompt("other-source").source("codex").session("s1").insert_unindexed(&store);

    assert_eq!(store.latest_turn("cursor", "s1").unwrap().as_deref(), Some("new"));
    assert_eq!(store.latest_turn("cursor", "nope").unwrap(), None);
}

#[test]
fn rejoin_names_unnamed_prompts_and_attaches_each_event_to_the_latest_prompt_at_or_before_it() {
    let store = store();
    let cursor = |at: &str| PromptBuilder::new().source("cursor").session("s1").at(at);
    let first = cursor("2026-11-03T10:00:00.000000+00:00").insert_unindexed(&store);
    let second = cursor("2026-11-03T10:01:00.000000+00:00").insert_unindexed(&store);
    let ev = |name: &str, at: &str| event(name).source("cursor").session("s1").at(at).insert(&store).id;
    let early = ev("before-any-prompt", "2026-11-03T09:59:00.000000+00:00");
    let in_first = ev("in-first", "2026-11-03T10:00:30.000000+00:00");
    let at_second = ev("at-second", "2026-11-03T10:01:00.000000+00:00");
    let after = ev("after", "2026-11-03T10:05:00.000000+00:00");
    let named = event("already").source("cursor").session("s1").prompt_id("kept").insert(&store).id;

    store.rejoin("cursor").unwrap();

    let (a, b) = (format!("cursor#{first}"), format!("cursor#{second}"));
    let joined: BTreeMap<i64, (Option<String>, Option<Json>)> =
        store.events_for_session("s1").unwrap().into_iter().map(|e| (e.id, (e.prompt_id, e.fields.get("prompt_id").cloned()))).collect();
    let expected = BTreeMap::from([
        (early, (None, None)),
        (in_first, (Some(a.clone()), Some(json!(a)))),
        (at_second, (Some(b.clone()), Some(json!(b)))),
        (after, (Some(b.clone()), Some(json!(b)))),
        (named, (Some("kept".into()), Some(json!("kept")))),
    ]);
    assert_eq!(joined, expected);
    assert_eq!(store.prompts_of_turns(&[a, b]).unwrap(), vec![first, second]);
}

#[test]
fn rejoin_leaves_other_sources_and_already_named_prompts_alone() {
    let store = store();
    let named = prompt("own-id").source("cursor").insert_unindexed(&store);
    let other = PromptBuilder::new().source("codex").insert_unindexed(&store);
    let stray = event("e").source("codex").insert(&store);

    store.rejoin("cursor").unwrap();

    assert_eq!(store.prompts(&[named]).unwrap()[0].fields["prompt_id"], json!("own-id"));
    assert_eq!(store.prompts(&[other]).unwrap()[0].fields.get("prompt_id"), None);
    assert_eq!(store.events_for_session("s1").unwrap(), vec![stray]);
}

#[test]
fn an_event_of_a_session_without_prompts_stays_unjoined() {
    let store = store();
    PromptBuilder::new().source("cursor").session("s1").insert_unindexed(&store);
    let lonely = event("e").source("cursor").session("s2").insert(&store);

    store.rejoin("cursor").unwrap();

    assert_eq!(store.events_for_session("s2").unwrap(), vec![lonely]);
}

// --- reads --------------------------------------------------------------------------------------

#[test]
fn search_returns_the_newest_prompts_first_up_to_the_limit() {
    let store = store();
    let ids: Vec<i64> = (0..4).map(|i| prompt(&format!("p{i}")).insert(&store)).collect();

    let listed: Vec<i64> = store.search(&PromptFilter::default(), 3).unwrap().iter().map(|p| p.row.id).collect();

    assert_eq!(listed, vec![ids[3], ids[2], ids[1]]);
}

#[test]
fn search_only_lists_prompts_that_are_indexed() {
    let store = store();
    let indexed = prompt("p1").insert(&store);
    prompt("p2").insert_unindexed(&store);

    assert_eq!(found(&store, &PromptFilter::default()), vec![indexed]);
}

#[test]
fn turns_count_the_prompts_of_the_same_source_and_session() {
    let store = store();
    let first = prompt("p1").session("a").at("2026-11-03T10:00:00.000000+00:00").insert(&store);
    prompt("p2").session("b").insert(&store);
    prompt("p3").session("a").source("codex").insert(&store);
    let second = prompt("p4").session("a").at("2026-11-03T10:05:00.000000+00:00").insert(&store);

    let view = store.get(second).unwrap().unwrap();

    assert_eq!(view.turn, 2);
    assert_eq!(view.prev_at.as_deref(), Some("2026-11-03T10:00:00.000000+00:00"));
    assert_eq!(store.get(first).unwrap().unwrap().prev_at, None);
    let a = PromptFilter { session: vec!["a".into()], source: Some("claude-code".into()), ..Default::default() };
    let turns: Vec<i64> = store.search(&a, 50).unwrap().iter().map(|p| p.turn).collect();
    assert_eq!(turns, vec![2, 1]);
}

#[test]
fn prompts_without_a_session_are_numbered_together_per_source() {
    let store = store();
    let lone: Vec<i64> = (0..2).map(|i| prompt(&format!("n{i}")).no_session().insert(&store)).collect();

    let turns: Vec<i64> = lone.iter().map(|id| store.get(*id).unwrap().unwrap().turn).collect();

    assert_eq!(turns, vec![1, 2]);
}

#[test]
fn next_head_is_the_git_head_at_the_next_prompt_of_the_session() {
    let store = store();
    let first = prompt("p1").context(json!({"git_head": "aaa"})).insert(&store);
    let second = prompt("p2").context(json!({"git_head": "bbb"})).insert(&store);

    assert_eq!(store.get(first).unwrap().unwrap().next_head.as_deref(), Some("bbb"));
    assert_eq!(store.get(second).unwrap().unwrap().next_head, None);
}

#[test]
fn get_returns_the_stored_row_with_an_empty_annotation_by_default() {
    let store = store();
    let id = prompt("p1").text("Fix it").insert(&store);

    let view = store.get(id).unwrap().unwrap();

    assert_eq!(view.row, store.prompts(&[id]).unwrap()[0]);
    assert_eq!((view.annotation, view.turn), (Annotation::default(), 1));
}

#[test]
fn get_of_an_unknown_id_is_none() {
    assert_eq!(store().get(42).unwrap(), None);
}

#[test]
fn prompts_skips_unknown_ids() {
    let store = store();
    let id = prompt("p1").insert_unindexed(&store);

    let rows: Vec<i64> = store.prompts(&[999, id]).unwrap().iter().map(|r| r.id).collect();

    assert_eq!(rows, vec![id]);
}

#[test]
fn prompts_of_turns_finds_the_row_ids_of_any_source() {
    let store = store();
    let a = prompt("p1").source("claude-code").insert_unindexed(&store);
    let b = prompt("p2").source("codex").insert_unindexed(&store);
    prompt("p3").insert_unindexed(&store);

    let mut ids = store.prompts_of_turns(&["p1".into(), "p2".into(), "unknown".into()]).unwrap();
    ids.sort_unstable();

    assert_eq!(ids, vec![a, b]);
}

#[test]
fn prompt_ids_lists_every_prompt_ascending() {
    let store = store();
    let ids: Vec<i64> = (0..3).map(|i| prompt(&format!("p{i}")).insert_unindexed(&store)).collect();
    assert_eq!(store.prompt_ids().unwrap(), ids);
}

#[test]
fn events_for_prompts_lists_only_those_turns_oldest_first() {
    let store = store();
    let a = event("PostToolUse").prompt_id("p1").insert(&store);
    event("Stop").prompt_id("other").insert(&store);
    let b = event("Stop").prompt_id("p1").insert(&store);
    let c = event("Stop").prompt_id("p2").insert(&store);

    assert_eq!(store.events_for_prompts(&["p1".into(), "p2".into()]).unwrap(), vec![a, b, c]);
    assert_eq!(store.events_for_prompts(&[]).unwrap(), vec![]);
}

#[test]
fn events_after_lists_newer_events_matching_the_prefix_or_a_name() {
    let store = store();
    let seen = event("claude_code.cost").insert(&store);
    let metric = event("claude_code.lines_of_code.count").insert(&store);
    event("Stop").insert(&store);
    let named = event("user_prompt").insert(&store);
    event("claude_codex").insert(&store);

    let after = store.events_after(seen.id, "claude_code.", &["user_prompt"]).unwrap();

    assert_eq!(after, vec![metric, named]);
}

#[test]
fn events_after_without_names_matches_the_prefix_only() {
    let store = store();
    let metric = event("claude_code.x").insert(&store);
    event("user_prompt").insert(&store);

    assert_eq!(store.events_after(0, "claude_code.", &[]).unwrap(), vec![metric]);
}

#[test]
fn events_for_session_lists_its_events_oldest_first() {
    let store = store();
    let a = event("a").session("s1").insert(&store);
    event("b").session("s2").insert(&store);
    let c = event("c").session("s1").insert(&store);

    assert_eq!(store.events_for_session("s1").unwrap(), vec![a, c]);
}

#[test]
fn samples_are_the_newest_events_then_each_newest_prompt_followed_by_its_context() {
    let store = store();
    event("old").raw(json!({"n": 0})).insert(&store);
    event("Stop").raw(json!({"n": 1})).insert(&store);
    event("x").unnamed().raw(json!({"n": 2})).insert(&store);
    prompt("p0").raw(json!({"old": true})).insert_unindexed(&store);
    prompt("p1").raw(json!({"p": 1})).context(json!({"git_head": "h"})).insert_unindexed(&store);

    let samples = store.samples(2).unwrap();

    let sample = |source: &str, event: &str, data: Json| Sample { source: source.into(), event: event.into(), data };
    assert_eq!(
        samples,
        vec![
            sample("claude-code", "?", json!({"n": 2})),
            sample("claude-code", "Stop", json!({"n": 1})),
            sample("claude-code", "UserPromptSubmit", json!({"p": 1})),
            sample("gitprompt", "context (at ingest)", json!({"git_head": "h"})),
            sample("claude-code", "UserPromptSubmit", json!({"old": true})),
            sample("gitprompt", "context (at ingest)", json!({})),
        ]
    );
}

#[test]
fn a_prompt_sample_without_an_event_field_is_named_with_a_question_mark() {
    let store = store();
    prompt("p1").without_field("event").raw(json!({})).insert_unindexed(&store);

    assert_eq!(store.samples(5).unwrap()[0].event, "?");
}

#[test]
fn source_stats_count_prompts_unmapped_prompts_and_events_per_source() {
    let store = store();
    prompt("p1").at("2026-11-03T10:00:00.000000+00:00").insert_unindexed(&store);
    prompt("p2").without_field("prompt").at("2026-11-03T11:00:00.000000+00:00").insert_unindexed(&store);
    event("Stop").at("2026-11-03T10:30:00.000000+00:00").insert(&store);
    event("api_request").source("otel").at("2026-11-03T09:00:00.000000+00:00").insert(&store);
    event("api_request").source("otel").at("2026-11-03T09:30:00.000000+00:00").insert(&store);

    let stats = store.source_stats().unwrap();

    let expected = BTreeMap::from([
        (
            "claude-code".to_string(),
            SourceStats { total: 2, unmapped: 1, events: 1, last_at: Some("2026-11-03T11:00:00.000000+00:00".into()) },
        ),
        ("otel".to_string(), SourceStats { total: 0, unmapped: 0, events: 2, last_at: Some("2026-11-03T09:30:00.000000+00:00".into()) }),
    ]);
    assert_eq!(stats, expected);
}

#[test]
fn hourly_counts_prompts_and_events_per_source_and_utc_hour_from_since_inclusive() {
    let store = store();
    prompt("before").at("2026-11-03T09:59:59.999999+00:00").insert_unindexed(&store);
    prompt("on").at("2026-11-03T10:00:00.000000+00:00").insert_unindexed(&store);
    prompt("later").at("2026-11-03T10:40:00.000000+00:00").insert_unindexed(&store);
    event("Stop").at("2026-11-03T10:50:00.000000+00:00").insert(&store);
    event("Stop").source("otel").at("2026-11-03T11:10:00.000000+00:00").insert(&store);

    let rows = store.hourly("2026-11-03T10:00:00.000000+00:00").unwrap();

    // Prompts and events may come as separate rows for one hour: compare the sums.
    let summed = rows.iter().fold(BTreeMap::new(), |mut acc, HourCount { source, hour, rows }| {
        *acc.entry((source.clone(), hour.clone())).or_insert(0) += rows;
        acc
    });
    let expected = BTreeMap::from([
        (("claude-code".to_string(), "2026-11-03T10".to_string()), 3),
        (("otel".to_string(), "2026-11-03T11".to_string()), 1),
    ]);
    assert_eq!(summed, expected);
}

// --- annotations --------------------------------------------------------------------------------

#[test]
fn an_annotation_reads_back_and_shows_on_the_prompt() {
    let store = store();
    let id = prompt("p1").insert(&store);
    let note = Annotation { starred: true, rating: 1, tags: vec!["release".into()], note: "good one".into() };

    store.annotate(id, &note).unwrap();

    assert_eq!(store.annotation(id).unwrap(), note);
    assert_eq!(store.get(id).unwrap().unwrap().annotation, note);
}

#[test]
fn annotating_again_replaces_the_annotation() {
    let store = store();
    let id = prompt("p1").insert(&store);
    store.annotate(id, &Annotation { starred: true, tags: vec!["a".into()], ..Default::default() }).unwrap();
    let second = Annotation { note: "only this".into(), ..Default::default() };

    store.annotate(id, &second).unwrap();

    assert_eq!(store.annotation(id).unwrap(), second);
}

#[test]
fn a_prompt_without_notes_has_the_empty_annotation() {
    assert_eq!(store().annotation(7).unwrap(), Annotation::default());
}

// --- rewrite ------------------------------------------------------------------------------------

#[test]
fn rewrite_edits_the_prompt_and_every_event_of_its_turn() {
    let store = store();
    let id = prompt("p1")
        .text("ship it, password=hunter2")
        .context(json!({"note": "hunter2"}))
        .raw(json!({"prompt": "ship it, password=hunter2"}))
        .insert(&store);
    event("PostToolUse").prompt_id("p1").raw(json!({"tool_input": {"content": "hunter2"}})).insert(&store);
    event("Stop").prompt_id("p1").field("last", json!("hunter2")).insert(&store);
    let other = event("Stop").prompt_id("p2").raw(json!({"x": "hunter2"})).insert(&store);

    let changed = store.rewrite(id, &replacing("hunter2", "[hidden]")).unwrap();

    assert_eq!(changed, 3);
    let row = &store.prompts(&[id]).unwrap()[0];
    assert_eq!(row.prompt.as_deref(), Some("ship it, password=[hidden]"));
    let stored = serde_json::to_string(&(row, store.events_for_prompts(&["p1".into()]).unwrap())).unwrap();
    assert!(!stored.contains("hunter2"), "{stored}");
    assert_eq!(store.events_for_prompts(&["p2".into()]).unwrap(), vec![other]);
}

#[test]
fn rewrite_of_a_prompt_without_prompt_id_changes_only_the_prompt() {
    let store = store();
    let id = PromptBuilder::new().text("secret").insert(&store);
    let event = event("Stop").insert(&store);

    assert_eq!(store.rewrite(id, &replacing("secret", "x")).unwrap(), 1);
    assert_eq!(store.events_for_session("s1").unwrap(), vec![event]);
}

#[test]
fn rewrite_of_an_unknown_prompt_changes_nothing() {
    assert_eq!(store().rewrite(42, &replacing("a", "b")).unwrap(), 0);
}

// --- remap --------------------------------------------------------------------------------------

#[test]
fn remap_reruns_the_mapping_over_every_payload_of_the_source() {
    let store = store();
    let id = prompt("p1").raw(json!({"text": "typed", "sid": "s9", "pid": "q1"})).insert_unindexed(&store);
    event("x").raw(json!({"name": "Stop", "sid": "s9", "pid": "q1"})).insert(&store);
    let untouched = event("y").source("codex").raw(json!({"name": "Stop"})).insert(&store);
    let mapper = |raw: &Json| {
        obj(json!({"prompt": raw.get("text"), "event": raw.get("name"), "session_id": raw.get("sid"), "prompt_id": raw.get("pid")}))
    };

    let remapped = store.remap("claude-code", &mapper).unwrap();

    assert_eq!(remapped, 2);
    assert_eq!(store.prompts(&[id]).unwrap()[0].prompt.as_deref(), Some("typed"));
    let [moved] = store.events_for_prompts(&["q1".into()]).unwrap().try_into().unwrap();
    assert_eq!((moved.event.as_deref(), moved.session_id.as_deref()), (Some("Stop"), Some("s9")));
    assert_eq!(moved.fields, mapper(&moved.raw));
    assert_eq!(store.events_for_session("s1").unwrap(), vec![untouched]);
}

#[test]
fn remap_that_finds_no_text_leaves_the_prompt_unmapped() {
    let store = store();
    let id = prompt("p1").insert_unindexed(&store);

    store.remap("claude-code", &|_raw: &Json| obj(json!({"prompt_id": "p1"}))).unwrap();

    assert_eq!(store.prompts(&[id]).unwrap()[0].prompt, None);
    assert_eq!(store.source_stats().unwrap()["claude-code"].unmapped, 1);
}

// --- chunked `in (...)` reads ------------------------------------------------------------------

#[test]
fn prompts_reads_more_ids_than_one_chunk() {
    let store = store();
    let ids: Vec<i64> = store.transaction(|s| Ok((0..=CHUNK).map(|i| prompt(&format!("p{i}")).insert_unindexed(s)).collect())).unwrap();

    let rows = store.prompts(&ids).unwrap();

    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
}

#[test]
fn events_for_prompts_reads_more_turns_than_one_chunk() {
    let store = store();
    let turns: Vec<String> = (0..=(CHUNK * 2)).map(|i| format!("p{i}")).collect();
    store
        .transaction(|s| {
            for t in &turns {
                event("Stop").prompt_id(t).insert(s);
            }
            Ok(())
        })
        .unwrap();

    let events = store.events_for_prompts(&turns).unwrap();

    assert_eq!(events.len(), turns.len());
    assert!(events.windows(2).all(|w| w[0].id < w[1].id), "oldest first");
}

#[test]
fn a_failed_transaction_error_is_returned_unchanged() {
    let result: Result<(), Error> = store().transaction(|_| Err(Error::Invalid("nope".into())));
    assert!(matches!(result, Err(Error::Invalid(m)) if m == "nope"));
}
