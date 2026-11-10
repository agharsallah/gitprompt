//! The prompt index: a rebuildable cache. Port of storage/index.py (create, drop, invalidate,
//! version, upsert, delete, telemetry, mark/take_marked, unindexed).

mod common;

use common::*;
use gitprompt_core::spec::ToolKind;
use gitprompt_store::PromptFilter;
use gitprompt_store::index::{IndexRow, SessionCommand, SessionMetric};
use gitprompt_store::stats::{CommandUse, Telemetry};
use pretty_assertions::assert_eq;

/// The row as `prompt_index` keeps it: the typed text lives in the full-text table only.
fn without_text(row: IndexRow) -> IndexRow {
    IndexRow { typed_text: None, ..row }
}

fn q(text: &str) -> PromptFilter {
    PromptFilter { q: Some(text.into()), ..Default::default() }
}

fn model(name: &str) -> PromptFilter {
    PromptFilter { model: vec![name.into()], ..Default::default() }
}

#[test]
fn create_is_idempotent() {
    let store = store();
    store.index().create().unwrap();
    store.index().create().unwrap();
    assert_eq!(store.index().version().unwrap(), None);
}

#[test]
fn the_rules_version_is_stored_and_replaced() {
    let store = store();
    store.index().set_version("v1").unwrap();
    store.index().set_version("v2").unwrap();
    assert_eq!(store.index().version().unwrap().as_deref(), Some("v2"));
}

#[test]
fn invalidating_forgets_the_rules_version_but_keeps_the_rows() {
    let store = store();
    let id = prompt("p1").insert(&store);
    store.index().set_version("v1").unwrap();

    store.index().invalidate().unwrap();

    assert_eq!(store.index().version().unwrap(), None);
    assert!(store.index().row(id).unwrap().is_some());
}

#[test]
fn an_upserted_row_reads_back_unchanged() {
    let store = store();
    let builder = prompt("p1")
        .text("Fix it")
        .with(|r| {
            r.repo = Some("/r/app".into());
            r.repo_name = Some("me/app".into());
            r.branch = Some("main".into());
            r.title = Some("Docs".into());
            r.text_hash = Some("h".into());
            r.slash_command = Some("/review".into());
            (r.code_blocks, r.urls, r.mentions, r.secrets) = (1, 2, 3, 4);
            r.usage.output_tokens = 7;
            r.usage.cache_read_tokens = 8;
            r.usage.cache_creation_tokens = 9;
            (r.api_time_s, r.api_errors, r.tool_failures, r.tool_time_s) = (1.5, 1, 1, 2.25);
            (r.subagent_tool_calls, r.subagents, r.files_written, r.git_commits) = (5, 1, 2, 1);
            (r.compactions, r.denials, r.rejections, r.reasoning_tokens) = (1, 2, 3, 40);
            r.permission_mode = Some("default".into());
            r.tool_version = Some("2.1.316".into());
            r.answered = true;
            r.surface = Some("cli".into());
        })
        .ended("Stop", 60.0)
        .model("opus", usage(2, 0.75, 10))
        .tool("Edit", ToolKind::Write, 1);
    let id = builder.insert_unindexed(&store);
    let entry = builder.entry_for(id);

    assert_eq!(store.index().upsert(std::slice::from_ref(&entry)).unwrap(), 1);

    assert_eq!(store.index().row(id).unwrap().map(without_text), Some(without_text(entry.row)));
}

#[test]
fn the_row_of_an_unindexed_prompt_is_none() {
    assert_eq!(store().index().row(1).unwrap(), None);
}

#[test]
fn upsert_counts_its_entries() {
    let store = store();
    let entries = vec![prompt("p1").entry_for(1), prompt("p2").entry_for(2), prompt("p3").entry_for(3)];
    assert_eq!(store.index().upsert(&entries).unwrap(), 3);
    assert_eq!(store.count(&PromptFilter::default()).unwrap(), 3);
}

#[test]
fn upsert_replaces_every_table_of_a_prompt() {
    let store = store();
    let before = prompt("p1").text("first words").model("opus", usage(1, 1.0, 0)).tool("Bash", ToolKind::Shell, 1);
    let id = before.insert(&store);
    let after = prompt("p1").text("second words").model("haiku", usage(1, 0.1, 0)).ended("Stop", 3.0);

    store.index().upsert(&[after.entry_for(id)]).unwrap();

    assert_eq!(store.index().row(id).unwrap().unwrap().ended_by.as_deref(), Some("Stop"));
    let count = |filter: PromptFilter| store.count(&filter).unwrap();
    assert_eq!((count(q("first")), count(q("second"))), (0, 1));
    assert_eq!((count(model("opus")), count(model("haiku"))), (0, 1));
    assert_eq!(count(PromptFilter { tool: Some("Bash".into()), ..Default::default() }), 0);
    assert_eq!(count(PromptFilter::default()), 1);
}

#[test]
fn an_entry_without_typed_text_is_indexed_but_not_searchable() {
    let store = store();
    let id = prompt("p1").with(|r| r.typed_text = None).insert(&store);

    assert!(store.index().row(id).unwrap().is_some());
    assert_eq!(store.count(&q("hello")).unwrap(), 0);
}

#[test]
fn deleting_removes_a_prompt_from_every_index_table() {
    let store = store();
    let gone = prompt("p1")
        .text("delete me")
        .model("opus", usage(1, 1.0, 0))
        .tool("Bash", ToolKind::Shell, 1)
        .work("main", None, usage(1, 1.0, 0))
        .insert(&store);
    let kept = prompt("p2").text("keep me").insert(&store);

    store.index().delete(&[gone]).unwrap();

    assert_eq!(store.index().row(gone).unwrap(), None);
    assert!(store.index().row(kept).unwrap().is_some());
    assert_eq!(store.count(&q("delete")).unwrap(), 0);
    assert_eq!(store.count(&model("opus")).unwrap(), 0);
    assert_eq!(store.count(&PromptFilter { tool_kind: Some(ToolKind::Shell), ..Default::default() }).unwrap(), 0);
    assert_eq!(store.prompt_ids().unwrap(), vec![gone, kept], "raw prompts are not the index");
}

#[test]
fn dropping_then_creating_gives_empty_tables() {
    let store = store();
    let id = prompt("p1").text("Fix it").insert(&store);
    store.index().set_version("v1").unwrap();
    store.index().mark(&["p1".into()]).unwrap();

    store.index().drop_all().unwrap();
    store.index().create().unwrap();

    assert_eq!(store.index().row(id).unwrap(), None);
    assert_eq!(store.index().version().unwrap(), None);
    assert_eq!(store.index().take_marked().unwrap(), Vec::<String>::new());
    assert_eq!(store.index().telemetry_after().unwrap(), 0);
    assert_eq!(store.count(&q("Fix")).unwrap(), 0);
}

#[test]
fn marked_turns_are_taken_once() {
    let store = store();
    store.index().mark(&["p1".into(), "p2".into()]).unwrap();
    store.index().mark(&["p1".into()]).unwrap();

    let mut taken = store.index().take_marked().unwrap();
    taken.sort();

    assert_eq!(taken, vec!["p1".to_string(), "p2".to_string()]);
    assert_eq!(store.index().take_marked().unwrap(), Vec::<String>::new());
}

#[test]
fn counting_marked_turns_leaves_them_marked() {
    let store = store();
    assert_eq!(store.index().marked_count().unwrap(), 0);
    store.index().mark(&["p1".into(), "p2".into(), "p1".into()]).unwrap();

    assert_eq!(store.index().marked_count().unwrap(), 2);
    assert_eq!(store.index().marked_count().unwrap(), 2);
    assert_eq!(store.index().take_marked().unwrap().len(), 2);
    assert_eq!(store.index().marked_count().unwrap(), 0);
}

#[test]
fn every_prompt_is_unindexed_before_the_first_index() {
    let store = store();
    let ids: Vec<i64> = (0..2).map(|i| prompt(&format!("p{i}")).insert_unindexed(&store)).collect();
    assert_eq!(store.index().unindexed().unwrap(), ids);
}

#[test]
fn unindexed_lists_prompts_stored_after_the_newest_indexed_one() {
    // Ported as is: a gap below the newest indexed prompt is not reported (ingest indexes in order).
    let store = store();
    prompt("p1").insert_unindexed(&store);
    prompt("p2").insert(&store);
    let newer = prompt("p3").insert_unindexed(&store);

    assert_eq!(store.index().unindexed().unwrap(), vec![newer]);
}

#[test]
fn telemetry_is_read_after_the_last_event_noted() {
    let store = store();
    assert_eq!(store.index().telemetry_after().unwrap(), 0);

    store.index().add_telemetry(41, &[], &[]).unwrap();

    assert_eq!(store.index().telemetry_after().unwrap(), 41);
}

#[test]
fn telemetry_data_points_added_twice_count_once() {
    let store = store();
    prompt("p1").session("s1").insert(&store);
    let metric = SessionMetric { event_id: 7, session: Some("s1".into()), received_at: T0.into(), name: "lines[added]".into(), value: 3.0 };
    let command = SessionCommand {
        event_id: 8,
        session: Some("s1".into()),
        received_at: T0.into(),
        command: "/model".into(),
        source: Some("builtin".into()),
    };

    store.index().add_telemetry(8, std::slice::from_ref(&metric), std::slice::from_ref(&command)).unwrap();
    store.index().add_telemetry(8, &[metric], &[command]).unwrap();

    let expected = Telemetry {
        sessions: 1,
        metrics: [("lines[added]".to_string(), 3.0)].into_iter().collect(),
        commands: vec![CommandUse { command: "/model".into(), source: Some("builtin".into()), uses: 1, sessions: 1 }],
    };
    assert_eq!(store.telemetry(&PromptFilter::default()).unwrap(), expected);
}
