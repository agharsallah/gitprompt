//! Reads for stories and search: prompts as the index sees them in time order, counts, tool kinds,
//! and the turn events a story shows. Port of storage/timeline.py.

mod common;

use std::collections::BTreeMap;

use common::*;
use gitprompt_core::spec::{Origin, ToolKind};
use gitprompt_store::store::CHUNK;
use gitprompt_store::timeline::TurnEvent;
use gitprompt_store::{Outcome, PromptFilter, Store};
use pretty_assertions::assert_eq;
use serde_json::json;

const NOW: &str = "2026-11-04T00:00:00.000000+00:00";

fn listed(store: &Store, filter: &PromptFilter, limit: Option<usize>, newest_first: bool) -> Vec<i64> {
    store.timeline(filter, limit, newest_first).unwrap().into_iter().map(|p| p.row.id).collect()
}

/// Three prompts stored out of time order: ids do not follow received_at.
fn out_of_order() -> (Store, [i64; 3]) {
    let store = store_at(NOW);
    let late = prompt("late").at("2026-11-03T12:00:00.000000+00:00").insert(&store);
    let early = prompt("early").at("2026-11-03T08:00:00.000000+00:00").insert(&store);
    let middle = prompt("middle").at("2026-11-03T10:00:00.000000+00:00").insert(&store);
    (store, [early, middle, late])
}

#[test]
fn the_timeline_is_oldest_first_by_received_at() {
    let (store, [early, middle, late]) = out_of_order();
    assert_eq!(listed(&store, &PromptFilter::default(), None, false), vec![early, middle, late]);
}

#[test]
fn the_timeline_can_be_newest_first() {
    let (store, [early, middle, late]) = out_of_order();
    assert_eq!(listed(&store, &PromptFilter::default(), None, true), vec![late, middle, early]);
}

#[test]
fn the_timeline_limit_keeps_the_first_in_its_order() {
    let (store, [early, middle, late]) = out_of_order();
    assert_eq!(listed(&store, &PromptFilter::default(), Some(2), false), vec![early, middle]);
    assert_eq!(listed(&store, &PromptFilter::default(), Some(1), true), vec![late]);
}

#[test]
fn prompts_received_at_the_same_time_are_ordered_by_id() {
    let store = store();
    let a = prompt("a").insert(&store);
    let b = prompt("b").insert(&store);

    assert_eq!(listed(&store, &PromptFilter::default(), None, false), vec![a, b]);
    assert_eq!(listed(&store, &PromptFilter::default(), None, true), vec![b, a]);
}

#[test]
fn the_timeline_takes_the_prompt_filters() {
    let store = store();
    let typed = prompt("p1").origin("typed").text("parser tests").insert(&store);
    prompt("p2").origin("task_notification").text("parser tests finished").insert(&store);
    prompt("p3").origin("typed").text("release notes").insert(&store);

    let filter = PromptFilter { q: Some("PARSER".into()), origin: vec![Origin::Typed], ..Default::default() };
    assert_eq!(listed(&store, &filter, None, false), vec![typed]);
}

#[test]
fn a_timeline_prompt_carries_its_index_row_typed_text_and_outcome() {
    let store = store_at(NOW);
    let builder = prompt("p1")
        .text("<ide_selection>x</ide_selection> Rename x")
        .typed("Rename x")
        .with(|r| r.repo = Some("/r/app".into()))
        .ended("StopFailure", 60.0)
        .tool("Edit", ToolKind::Write, 1);
    let id = builder.insert(&store);

    let [one] = store.timeline(&PromptFilter::default(), None, false).unwrap().try_into().unwrap();

    assert_eq!(one.row, builder.entry_for(id).row);
    assert_eq!(one.outcome, Outcome::Failed);
}

#[test]
fn timeline_one_reads_one_prompt_as_the_timeline_does() {
    let store = store_at(NOW);
    let id = prompt("p1").text("Fix it").insert(&store);
    prompt("p2").text("Other").insert(&store);

    let one = store.timeline_one(id).unwrap().unwrap();

    assert_eq!(Some(one.clone()), store.timeline(&PromptFilter::default(), Some(1), false).unwrap().into_iter().next());
    assert_eq!((one.row.typed_text.as_deref(), one.outcome), (Some("Fix it"), Outcome::Superseded));
}

#[test]
fn timeline_one_of_an_unindexed_prompt_is_none() {
    let store = store();
    let id = prompt("p1").insert_unindexed(&store);
    assert_eq!(store.timeline_one(id).unwrap(), None);
}

#[test]
fn count_counts_the_prompts_the_filter_keeps() {
    let (store, _) = out_of_order();
    let since = PromptFilter { since: Some("2026-11-03T10:00:00.000000+00:00".into()), ..Default::default() };

    assert_eq!(store.count(&PromptFilter::default()).unwrap(), 3);
    assert_eq!(store.count(&since).unwrap(), 2);
}

#[test]
fn tool_kinds_sums_calls_per_kind_for_each_prompt() {
    let store = store();
    let busy = prompt("p1").tool("Read", ToolKind::Read, 2).tool("Grep", ToolKind::Read, 3).tool("Bash", ToolKind::Shell, 1).insert(&store);
    let quiet = prompt("p2").insert(&store);
    let writer = prompt("p3").tool("Edit", ToolKind::Write, 4).insert(&store);

    let kinds = store.tool_kinds(&[busy, quiet, writer]).unwrap();

    let expected = BTreeMap::from([
        (busy, BTreeMap::from([(ToolKind::Read, 5), (ToolKind::Shell, 1)])),
        (writer, BTreeMap::from([(ToolKind::Write, 4)])),
    ]);
    assert_eq!(kinds, expected);
}

#[test]
fn tool_kinds_reads_more_prompts_than_one_chunk() {
    let store = store();
    let entries: Vec<_> =
        (1..=(CHUNK as i64 + 1)).map(|id| prompt(&format!("p{id}")).tool("Read", ToolKind::Read, 1).entry_for(id)).collect();
    store.index().upsert(&entries).unwrap();
    let ids: Vec<i64> = entries.iter().map(|e| e.row.id).collect();

    assert_eq!(store.tool_kinds(&ids).unwrap().len(), CHUNK + 1);
}

// --- turn events --------------------------------------------------------------------------------

fn turn_event(prompt_id: &str, event: &str, received_at: &str) -> TurnEvent {
    TurnEvent {
        prompt_id: prompt_id.into(),
        event: event.into(),
        received_at: received_at.into(),
        tool: None,
        tool_input: None,
        description: None,
        subagent_type: None,
        prompt: None,
    }
}

#[test]
fn turn_events_are_the_tool_calls_and_compactions_of_the_turns_oldest_first() {
    let store = store();
    let at = |s: u32| format!("2026-11-03T10:00:{s:02}.000000+00:00");
    event("PostToolUse").prompt_id("p1").at(&at(1)).raw(json!({"tool_name": "Edit", "tool_input": {"file_path": "/a"}})).insert(&store);
    event("Stop").prompt_id("p1").at(&at(2)).insert(&store);
    event("PreCompact").prompt_id("p2").at(&at(3)).insert(&store);
    event("PreToolUse").prompt_id("p1").at(&at(4)).insert(&store);
    event("PostToolUse").prompt_id("other").at(&at(5)).insert(&store);

    let events = store.turn_events(&["p1".into(), "p2".into()], 600).unwrap();

    let edit =
        TurnEvent { tool: Some("Edit".into()), tool_input: Some(json!({"file_path": "/a"})), ..turn_event("p1", "PostToolUse", &at(1)) };
    assert_eq!(events, vec![edit, turn_event("p2", "PreCompact", &at(3))]);
}

#[test]
fn a_subagent_call_shows_its_description_type_and_brief() {
    let store = store();
    let input = json!({"description": "Find helpers", "subagent_type": "Explore", "prompt": "List the test helpers"});
    event("PostToolUse").prompt_id("p1").at(T0).raw(json!({"tool_name": "Agent", "tool_input": input})).insert(&store);

    let [call] = store.turn_events(&["p1".into()], 600).unwrap().try_into().unwrap();

    let expected = TurnEvent {
        tool: Some("Agent".into()),
        tool_input: Some(input),
        description: Some("Find helpers".into()),
        subagent_type: Some("Explore".into()),
        prompt: Some("List the test helpers".into()),
        ..turn_event("p1", "PostToolUse", T0)
    };
    assert_eq!(call, expected);
}

#[test]
fn a_subagent_brief_falls_back_to_task_name_and_message_and_is_clipped_in_characters() {
    let store = store();
    let input = json!({"task_name": "explore", "message": "ééééé-more"});
    event("PostToolUse").prompt_id("p1").at(T0).raw(json!({"tool_name": "spawn_agent", "tool_input": input})).insert(&store);

    let [call] = store.turn_events(&["p1".into()], 5).unwrap().try_into().unwrap();

    assert_eq!((call.description.as_deref(), call.prompt.as_deref()), (Some("explore"), Some("ééééé")));
}

#[test]
fn a_tool_input_that_is_not_an_object_is_left_out() {
    let store = store();
    event("PostToolUse").prompt_id("p1").raw(json!({"tool_name": "Bash", "tool_input": "ls"})).insert(&store);

    let [call] = store.turn_events(&["p1".into()], 600).unwrap().try_into().unwrap();

    assert_eq!((call.tool.as_deref(), call.tool_input), (Some("Bash"), None));
}

#[test]
fn turn_events_read_more_turns_than_one_chunk() {
    let store = store();
    let turns: Vec<String> = (0..=CHUNK).map(|i| format!("p{i}")).collect();
    store
        .transaction(|s| {
            turns.iter().for_each(|t| {
                event("PreCompact").prompt_id(t).insert(s);
            });
            Ok(())
        })
        .unwrap();

    assert_eq!(store.turn_events(&turns, 600).unwrap().len(), CHUNK + 1);
}
