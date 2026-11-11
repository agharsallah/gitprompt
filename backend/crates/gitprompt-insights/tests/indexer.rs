//! The prompt index (indexer.rs): the projection of the derivations it stores, the rules version that
//! rebuilds it, and how it follows new prompts, new turn events and telemetry. Port of
//! `tests/insights/test_indexer.py`: projections are checked purely with `entry`, the update rules on an
//! in-memory store; the timezone is injected (never the process `TZ`). FTS search and filters are the
//! store's tests.
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

use std::sync::Arc;

use jiff::tz::Offset;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::{Berlin2026, bundled, event, event_at, obj, offset_hours, prompt, spec, specs, specs_with_override, usage, utc};
use gitprompt_core::Error;
use gitprompt_core::spec::{Pattern, Spec, ToolKind};
use gitprompt_insights::Specs;
use gitprompt_insights::derivations::{Derived, derive};
use gitprompt_insights::indexer::{CODE_VERSION, Indexer, entry, prompt_facts, rules_version, slash, text_hash};
use gitprompt_insights::zone::MachineZone;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::index::{ModelUsage, ToolCalls, UsageTotals, WorkUsage};
use gitprompt_store::{EventRow, Store};

fn claude_derived(view: gitprompt_store::PromptView, events: &[EventRow]) -> Derived {
    derive(&view, events, Some(&spec("claude-code")))
}

fn with_spec(specs: &Specs, change: impl FnOnce(&mut Spec)) -> Specs {
    let mut claude = (*specs["claude-code"]).clone();
    change(&mut claude);
    let mut out = specs.clone();
    out.insert("claude-code".into(), Arc::new(claude));
    out
}

// --- pure projections -----------------------------------------------------------------------------

#[test]
fn equal_texts_up_to_case_and_spacing_share_a_hash() {
    assert_eq!(text_hash(Some("Fix  the\nBug")), text_hash(Some("fix the bug")));
    assert_ne!(text_hash(Some("fix the bug")), text_hash(Some("fix the bugs")));
    assert!(text_hash(Some("x")).is_some_and(|h| h.len() == 16 && h.chars().all(|c| c.is_ascii_hexdigit())));
}

#[test]
fn no_text_or_only_whitespace_has_no_hash() {
    assert_eq!((text_hash(None), text_hash(Some("")), text_hash(Some(" \n "))), (None, None, None));
}

#[test]
fn slash_commands_get_exactly_one_leading_slash() {
    assert_eq!(
        [slash(Some("review")), slash(Some("/review")), slash(Some("//review"))],
        [Some("/review".to_string()), Some("/review".to_string()), Some("/review".to_string())]
    );
    assert_eq!((slash(None), slash(Some(""))), (None, None));
}

#[test]
fn the_index_row_is_what_the_api_derives() {
    let view = prompt("<ide_selection>x</ide_selection>\nRename x").field("cwd", json!("/work/app")).build();
    let events = [
        event("PostToolUse", json!({"tool_name": "Edit", "tool_input": {"file_path": "/a.py"}})),
        event("PostToolUse", json!({"tool_name": "mcp__gh__issue"})),
        event("api_request", json!({"model": "opus", "cost_usd": 0.5, "input_tokens": 10})),
        event("api_request", json!({"model": "haiku", "cost_usd": 0.25, "output_tokens": 4})),
        event_at("Stop", "2026-11-03T10:01:00+00:00", json!({})),
    ];
    let indexed = entry(&claude_derived(view, &events), &Berlin2026).unwrap();
    let row = &indexed.row;
    assert_eq!((row.origin.as_str(), row.typed_text.as_deref(), row.repo.as_deref()), ("typed", Some("Rename x"), Some("/work/app")));
    assert_eq!((row.day.as_str(), row.hour, row.weekday), ("2026-11-03", 11, 2));
    assert_eq!(
        (row.usage.cost_usd, row.usage.api_requests, row.response_s, row.ended_by.as_deref(), row.tool_calls),
        (0.75, 2, Some(60.0), Some("Stop"), 2)
    );
    let mut models = indexed.models.clone();
    models.sort_by(|a, b| a.model.cmp(&b.model));
    assert_eq!(
        models,
        vec![
            ModelUsage { model: Some("haiku".into()), usage: usage(1, 0, 4, 0, 0.25) },
            ModelUsage { model: Some("opus".into()), usage: usage(1, 10, 0, 0, 0.5) }
        ]
    );
    assert_eq!(
        indexed.tools,
        vec![
            ToolCalls { tool: Some("Edit".into()), kind: ToolKind::Write, calls: 1 },
            ToolCalls { tool: Some("mcp__gh__issue".into()), kind: ToolKind::Mcp, calls: 1 }
        ]
    );
}

#[test]
fn the_index_keeps_what_the_insights_need() {
    let view =
        prompt("<ide_selection>```py\nx\n```</ide_selection>\nSee https://a.b and @x.py").field("session_title", json!("Docs")).build();
    let api = |source: &str, cost: f64| {
        event("api_request", json!({"model": "opus", "cost_usd": cost, "query_source": source, "duration_ms": 500}))
    };
    let events = [
        event("PostToolUse", json!({"tool_name": "Bash", "tool_input": {"command": "git add . && git commit -m x"}, "agent_id": "a1"})),
        event("PostToolUse", json!({"tool_name": "Bash", "tool_input": {"command": "git log --grep commit"}})),
        event("PostToolUse", json!({"tool_name": "Write", "tool_input": {"file_path": "/a"}, "duration_ms": 2000})),
        event("PermissionDenied", json!({"tool_name": "Bash"})),
        event("PreCompact", json!({})),
        event("SubagentStart", json!({"agent_type": "Explore"})),
        event("user_prompt", json!({"command_name": "effort"})),
        api("repl_main_thread", 1.0),
        api("agent:builtin:Explore", 0.5),
        api("compact", 0.25),
    ];
    let indexed = entry(&claude_derived(view, &events), &utc()).unwrap();
    let row = &indexed.row;
    assert_eq!((row.title.as_deref(), row.slash_command.as_deref()), (Some("Docs"), Some("/effort")));
    assert_eq!((row.code_blocks, row.urls, row.mentions), (0, 1, 1));
    assert_eq!((row.git_commits, row.subagent_tool_calls, row.subagents, row.files_written), (1, 1, 1, 1));
    assert_eq!((row.denials, row.compactions, row.tool_time_s, row.api_time_s), (1, 1, 2.0, 1.5));
    assert_eq!(row.text_hash, text_hash(Some("see HTTPS://a.b   and @x.py")));
    let work: Vec<(String, Option<String>, f64)> =
        indexed.work.iter().map(|w| (w.work.clone(), w.query_source.clone(), w.usage.cost_usd)).collect();
    assert_eq!(
        work,
        vec![
            ("main".into(), Some("repl_main_thread".into()), 1.0),
            ("subagent".into(), Some("agent:builtin:Explore".into()), 0.5),
            ("system".into(), Some("compact".into()), 0.25)
        ]
    );
}

#[test]
fn a_hook_model_counts_its_prompt_when_no_model_call_reports_one() {
    let indexed =
        entry(&derive(&prompt("a").field("model", json!("gpt-5-codex")).source("codex").build(), &[], Some(&spec("codex"))), &utc())
            .unwrap();
    assert_eq!(
        (indexed.models, indexed.row.model.as_deref()),
        (vec![ModelUsage { model: Some("gpt-5-codex".into()), usage: UsageTotals::default() }], Some("gpt-5-codex"))
    );
}

#[test]
fn model_calls_win_over_the_hook_model() {
    let view = prompt("b").field("model", json!("ignored")).build();
    let indexed = entry(&claude_derived(view, &[event("api_request", json!({"model": "opus", "cost_usd": 0.5}))]), &utc()).unwrap();
    assert_eq!(
        (indexed.models, indexed.row.model.as_deref()),
        (vec![ModelUsage { model: Some("opus".into()), usage: usage(1, 0, 0, 0, 0.5) }], Some("opus"))
    );
}

#[test]
fn secrets_are_counted_in_the_prompt_as_received_and_size_in_the_typed_text() {
    let indexed =
        entry(&claude_derived(prompt("<ide_selection>password=hunter2hunter2</ide_selection>\nhi").build(), &[]), &utc()).unwrap();
    assert_eq!((indexed.row.secrets, indexed.row.chars), (1, 2));
}

#[test]
fn the_git_context_names_the_repo_and_wins_over_the_working_directory() {
    let view = prompt("x")
        .field("cwd", json!("/w/shop/pkg"))
        .context("git_root", json!("/w/shop"))
        .context("git_repo", json!("acme/shop"))
        .context("git_branch", json!("main"))
        .build();
    let row = entry(&claude_derived(view, &[]), &utc()).unwrap().row;
    assert_eq!((row.repo.as_deref(), row.repo_name.as_deref(), row.branch.as_deref()), (Some("/w/shop"), Some("acme/shop"), Some("main")));
}

#[test]
fn the_tool_version_the_tool_reported_wins_over_the_probed_one() {
    let reported = prompt("x").field("tool_version", json!("3.4.1")).context("tool_version", json!("9.9.9")).build();
    let probed = prompt("x").context("tool_version", json!("2.1.316")).build();
    let versions = [reported, probed].map(|v| entry(&claude_derived(v, &[]), &utc()).unwrap().row.tool_version);
    assert_eq!(versions, [Some("3.4.1".to_string()), Some("2.1.316".to_string())]);
}

#[test]
fn mapped_fields_name_the_turn_session_and_permission_mode() {
    let view = prompt("x").field("prompt_id", json!("p1")).field("session_id", json!("s1")).field("permission_mode", json!("plan")).build();
    let row = entry(&claude_derived(view, &[]), &utc()).unwrap().row;
    assert_eq!(
        (row.prompt_id.as_deref(), row.session.as_deref(), row.permission_mode.as_deref(), row.id, row.source.as_str()),
        (Some("p1"), Some("s1"), Some("plan"), 1, "claude-code")
    );
}

#[test]
fn a_typed_slash_command_is_kept_when_no_event_names_one() {
    let row = entry(&claude_derived(prompt("/review this").build(), &[]), &utc()).unwrap().row;
    assert_eq!((row.slash_command.as_deref(), row.origin.as_str()), (Some("/review"), "slash_command"));
}

#[test]
fn an_answered_turn_and_rejected_tools_are_recorded() {
    let events = [event("Stop", json!({"last_assistant_message": "Done."})), event("tool_decision", json!({"decision": "reject"}))];
    let row = entry(&claude_derived(prompt("x").build(), &events), &utc()).unwrap().row;
    assert_eq!((row.answered, row.rejections), (true, 1));
}

#[test]
fn an_unfinished_turn_is_not_answered_and_has_no_end() {
    let row = entry(&claude_derived(prompt("x").build(), &[]), &utc()).unwrap().row;
    assert_eq!((row.answered, row.ended_by, row.response_s, row.model), (false, None, None, None));
}

#[test]
fn calls_of_a_tool_without_a_name_are_other_tools() {
    let indexed = entry(&claude_derived(prompt("x").build(), &[event("PostToolUse", json!({}))]), &utc()).unwrap();
    assert_eq!((indexed.tools, indexed.row.tool_calls), (vec![ToolCalls { tool: None, kind: ToolKind::Other, calls: 1 }], 1));
}

#[test]
fn work_without_a_query_source_is_the_main_thread() {
    let indexed = entry(&claude_derived(prompt("x").build(), &[event("api_request", json!({"model": "opus"}))]), &utc()).unwrap();
    assert_eq!(indexed.work, vec![WorkUsage { work: "main".into(), query_source: None, usage: usage(1, 0, 0, 0, 0.0) }]);
}

#[test]
fn time_buckets_follow_the_injected_zone() {
    let derived = claude_derived(prompt("x").at("2026-11-03T23:30:00+00:00").build(), &[]);
    let buckets = |zone: &Offset| {
        let r = entry(&derived, zone).unwrap().row;
        (r.day, r.hour, r.weekday)
    };
    assert_eq!((buckets(&utc()), buckets(&offset_hours(2))), (("2026-11-03".to_string(), 23, 2), ("2026-11-04".to_string(), 1, 3)));
}

#[test]
fn an_unreadable_received_at_is_an_invalid_input() {
    let derived = claude_derived(prompt("x").at("someday").build(), &[]);
    assert!(matches!(entry(&derived, &utc()), Err(Error::Invalid(_))));
}

#[test]
fn prompt_facts_list_every_named_model_of_the_turn() {
    let events = [
        event("api_request", json!({"model": "opus"})),
        event("api_request", json!({"model": "opus"})),
        event("api_request", json!({"model": "haiku"})),
        event("api_request", json!({})),
    ];
    let facts = prompt_facts(&claude_derived(prompt("x").build(), &events), Some(&spec("claude-code")), None);
    let mut models = facts.models.clone();
    models.sort();
    assert_eq!((facts.model.as_deref(), models, facts.api_requests), (Some("opus"), vec!["haiku".to_string(), "opus".to_string()], 4));
}

// --- the rules version ----------------------------------------------------------------------------

#[test]
fn the_rules_version_is_a_stable_short_hash() {
    let all = specs();
    let version = rules_version(&all, &utc(), CODE_VERSION);
    assert_eq!(version, rules_version(&specs(), &utc(), CODE_VERSION));
    assert!(version.len() == 16 && version.chars().all(|c| c.is_ascii_hexdigit()), "{version}");
}

#[test]
fn a_change_of_any_rule_changes_the_rules_version() {
    let all = specs();
    let base = rules_version(&all, &utc(), CODE_VERSION);
    let changed = [
        with_spec(&all, |s| {
            s.tool_kinds.insert("Bash".into(), ToolKind::Web);
        }),
        with_spec(&all, |s| s.origin.reverse()),
        with_spec(&all, |s| s.surface.reverse()),
        with_spec(&all, |s| s.injected.push("extra".into())),
        with_spec(&all, |s| {
            s.tool_files.insert("x".into(), Pattern::new("(.+)").unwrap());
        }),
        with_spec(&all, |s| s.usage.input_includes_cached = true),
    ]
    .map(|specs| rules_version(&specs, &utc(), CODE_VERSION));
    let unchanged: Vec<&String> = changed.iter().filter(|v| **v == base).collect();
    assert_eq!(unchanged, Vec::<&String>::new());
}

#[test]
fn data_that_is_not_a_rule_keeps_the_rules_version() {
    let all = specs();
    let renamed = with_spec(&all, |s| {
        s.name = "Renamed".into();
        s.summary = "other".into();
    });
    assert_eq!(rules_version(&renamed, &utc(), CODE_VERSION), rules_version(&all, &utc(), CODE_VERSION));
}

#[test]
fn the_timezone_and_the_code_version_are_part_of_the_rules_version() {
    let all = specs();
    let base = rules_version(&all, &utc(), CODE_VERSION);
    assert_ne!(rules_version(&all, &offset_hours(1), CODE_VERSION), base);
    assert_ne!(rules_version(&all, &utc(), "changed"), base);
}

#[test]
fn a_default_indexer_uses_the_machine_zone_and_the_code_version() {
    let all = specs();
    assert_eq!(Indexer::new(all.clone()).version(), rules_version(&all, &MachineZone, CODE_VERSION));
}

// --- keeping a store's index up to date -------------------------------------------------------------

fn indexer() -> Indexer {
    Indexer::with(specs(), Arc::new(utc()), CODE_VERSION)
}

fn add_prompt(store: &Store, text: &str, pid: &str, session: Option<&str>) -> i64 {
    let fields = obj(json!({"prompt": text, "prompt_id": pid, "session_id": session}));
    store.add_prompt("claude-code", &fields, &obj(json!({})), &json!({"prompt": text})).expect("stored").expect("not a duplicate").id
}

/// An event of a turn, marked as ingest does.
fn add_event(store: &Store, source: &str, name: &str, pid: &str, raw: Value) {
    store.add_event(source, &obj(json!({"event": name, "prompt_id": pid, "session_id": "s1"})), &raw).expect("stored");
    store.index().mark(&[pid.to_string()]).expect("marked");
}

fn row(store: &Store, id: i64) -> gitprompt_store::index::IndexRow {
    store.index().row(id).expect("read").expect("indexed")
}

#[test]
fn ensure_indexes_new_prompts_once() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    let id = add_prompt(&store, "<ide_selection>x</ide_selection> Fix it", "p1", Some("s1"));
    add_prompt(&store, "Other", "p2", Some("s1"));
    assert_eq!((index.ensure(&store).unwrap(), index.ensure(&store).unwrap()), (2, 0));
    assert_eq!((row(&store, id).typed_text.as_deref(), row(&store, id).origin.as_str()), (Some("Fix it"), "typed"));
}

#[test]
fn the_stored_row_is_the_projection_of_the_derivations() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    let id = add_prompt(&store, "Fix it", "p1", Some("s1"));
    add_event(&store, "claude-code", "Stop", "p1", json!({"last_assistant_message": "Done."}));
    index.ensure(&store).unwrap();
    let view = store.get(id).unwrap().expect("stored");
    let events = store.events_for_prompts(&["p1".to_string()]).unwrap();
    assert_eq!(row(&store, id), entry(&claude_derived(view, &events), &utc()).unwrap().row);
    assert_eq!(index.entries(&store, &[id, 999]).unwrap().len(), 1);
}

#[test]
fn ensure_stamps_the_index_with_its_rules_version() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    index.ensure(&store).unwrap();
    assert_eq!(store.index().version().unwrap().as_deref(), Some(index.version()));
}

#[test]
fn a_later_event_updates_its_prompt_only_once_read() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    let id = add_prompt(&store, "Fix it", "p1", Some("s1"));
    let other = add_prompt(&store, "Other", "p2", Some("s1"));
    index.ensure(&store).unwrap();
    assert_eq!(row(&store, id).ended_by, None);

    add_event(&store, "claude-code", "Stop", "p1", json!({}));
    add_event(&store, "claude-code-otel", "api_request", "p1", json!({"model": "opus", "cost_usd": 1.5}));
    assert_eq!(row(&store, id).ended_by, None, "ingest only marks the turn");
    assert_eq!(index.ensure(&store).unwrap(), 1, "the next read recomputes it, once, and nothing else");
    assert_eq!((row(&store, id).ended_by.as_deref(), row(&store, id).usage.cost_usd), (Some("Stop"), 1.5));
    assert_eq!((row(&store, other).ended_by, index.ensure(&store).unwrap()), (None, 0));
}

#[test]
fn events_before_their_prompt_are_counted_when_it_arrives() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    add_event(&store, "claude-code-otel", "user_prompt", "p1", json!({"command_name": "/review"}));
    let id = add_prompt(&store, "Review this", "p1", Some("s1"));
    index.ensure(&store).unwrap();
    assert_eq!(row(&store, id).origin, "slash_command");
}

#[test]
fn a_rule_change_rebuilds_the_index_once() {
    let store = Store::open_in_memory().unwrap();
    let id = add_prompt(&store, "BUILD: nightly", "p1", Some("s1"));
    let index = indexer();
    index.ensure(&store).unwrap();
    assert_eq!(row(&store, id).origin, "typed");

    let rule = "origin:\n  - when: {text_prefix: \"BUILD:\"}\n    origin: programmatic\n";
    let changed = Indexer::with(
        specs_with_override("claude-code.yaml", &bundled("claude-code.yaml").replacen("origin:\n", rule, 1)),
        Arc::new(utc()),
        CODE_VERSION,
    );
    assert_ne!(changed.version(), index.version());
    assert_eq!((changed.ensure(&store).unwrap(), row(&store, id).origin.as_str()), (1, "programmatic"));
    assert_eq!(changed.ensure(&store).unwrap(), 0, "rebuilt once");
}

#[test]
fn a_code_version_change_rebuilds_the_index() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(&store, "Fix it", "p1", Some("s1"));
    indexer().ensure(&store).unwrap();
    assert_eq!(Indexer::with(specs(), Arc::new(utc()), "changed").ensure(&store).unwrap(), 1);
}

#[test]
fn a_timezone_change_rebuilds_the_index_and_moves_the_hour() {
    let store = Store::open_in_memory().unwrap();
    let id = add_prompt(&store, "Fix it", "p1", Some("s1"));
    indexer().ensure(&store).unwrap();
    let utc_hour = row(&store, id).hour;
    let east = Indexer::with(specs(), Arc::new(offset_hours(5)), CODE_VERSION);
    assert_eq!((east.ensure(&store).unwrap(), row(&store, id).hour), (1, (utc_hour + 5) % 24));
}

#[test]
fn a_dropped_index_is_rebuilt_from_the_raw_rows() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    let id = add_prompt(&store, "Fix it", "p1", Some("s1"));
    index.ensure(&store).unwrap();
    store.index().drop_all().unwrap();
    assert_eq!((index.rebuild(&store).unwrap(), row(&store, id).typed_text.as_deref()), (1, Some("Fix it")));
}

#[test]
fn reindex_counts_each_prompt_once() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    let id = add_prompt(&store, "Fix it", "p1", Some("s1"));
    index.ensure(&store).unwrap();
    assert_eq!((index.reindex(&store, &[id, id]).unwrap(), index.reindex(&store, &[]).unwrap()), (1, 0));
}

#[test]
fn session_telemetry_is_read_incrementally_and_never_counted_twice() {
    let store = Store::open_in_memory().unwrap();
    let index = indexer();
    add_prompt(&store, "hello", "p1", Some("s1"));
    let metric = |value: i64| {
        store
            .add_event(
                "claude-code-otel",
                &obj(json!({"event": "claude_code.lines_of_code.count", "session_id": "s1"})),
                &json!({"type": "added", "value": value}),
            )
            .unwrap()
    };
    metric(3);
    let last = metric(4);
    index.ensure(&store).unwrap();
    store
        .add_event(
            "claude-code-otel",
            &obj(json!({"event": "user_prompt", "session_id": "s1"})),
            &json!({"command_name": "model", "command_source": "builtin"}),
        )
        .unwrap();
    index.ensure(&store).unwrap();
    assert!(store.index().telemetry_after().unwrap() > last.id);
    index.rebuild(&store).unwrap();
    let telemetry = store.telemetry(&PromptFilter::default()).unwrap();
    assert_eq!(telemetry.metrics.get("claude_code.lines_of_code.count[added]"), Some(&7.0));
    assert_eq!(
        telemetry.commands.iter().map(|c| (c.command.as_str(), c.source.as_deref())).collect::<Vec<_>>(),
        vec![("/model", Some("builtin"))]
    );
}
