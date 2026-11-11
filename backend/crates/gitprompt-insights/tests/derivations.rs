//! Derived values (derivations.rs): prompt stats, the turn summary, the session summary and
//! `derive`. Port of `tests/insights/test_insights.py` (non-secret parts) and the derivation
//! behaviour `tests/api/test_api.py` and `test_codex.py` assert through the API, plus every branch of
//! derivations.py they left implicit. Fixture-based cases replay tests' connector fixtures.
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

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::json;

use common::{all_at, event, event_at, obj, prompt, replay, spec, usage};
use gitprompt_core::Json;
use gitprompt_core::spec::{Pattern, ToolKind, Usage, UsageValue};
use gitprompt_insights::derivations::{
    self, ModelCall, PromptStats, SubagentResult, SubagentTask, TurnError, TurnSummary, WorkKind, command, derive, files_of, metric,
    model_calls, prompt_stats, session, turn, usage_of, work_kind,
};
use gitprompt_store::EventRow;
use gitprompt_store::index::UsageTotals;

fn claude_turn(events: &[EventRow]) -> TurnSummary {
    turn(&prompt("x").build(), events, Some(&spec("claude-code")))
}

fn tool(name: &str, input: Json) -> EventRow {
    event("PostToolUse", json!({"tool_name": name, "tool_input": input}))
}

fn named(entries: &[(&str, u64)]) -> IndexMap<Option<String>, u64> {
    entries.iter().map(|(k, n)| (Some(k.to_string()), *n)).collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(std::string::ToString::to_string).collect()
}

fn patterns(entries: &[(&str, &str)]) -> IndexMap<String, Pattern> {
    entries.iter().map(|(k, p)| (k.to_string(), Pattern::new(p).unwrap())).collect()
}

const PATCH: &str = "*** Begin Patch\n*** Add File: a.txt\n+x\n*** Delete File: b.txt\n*** End Patch";
const PATCH_FILES: &str = r"(?m)^\*\*\* (?:Add|Update|Delete) File: (.+)$";

// --- prompt stats ----------------------------------------------------------------------------------

#[test]
fn prompt_stats_find_the_command_mentions_urls_and_code() {
    let stats = prompt_stats("/review @src/app.py see https://example.com/x?\n```py\nx\n```");
    assert_eq!(stats.slash_command.as_deref(), Some("/review"));
    assert_eq!((stats.at_mentions, stats.urls), (strings(&["src/app.py"]), strings(&["https://example.com/x?"])));
    assert_eq!((stats.code_blocks, stats.lines, stats.secrets), (1, 4, Vec::<String>::new()));
}

#[test]
fn prompt_size_counts_characters_words_lines_and_approximate_tokens() {
    let stats = prompt_stats("a b\nc");
    assert_eq!((stats.chars, stats.words, stats.lines, stats.approx_tokens), (5, 3, 2, 2));
}

#[test]
fn approximate_tokens_round_up_a_quarter_of_the_characters() {
    assert_eq!((prompt_stats("abcd").approx_tokens, prompt_stats("abcde").approx_tokens), (1, 2));
}

#[test]
fn characters_are_counted_not_bytes() {
    assert_eq!(prompt_stats("héllo").chars, 5);
}

#[test]
fn an_empty_text_has_no_lines_and_no_words() {
    assert_eq!(prompt_stats(""), PromptStats::default());
}

#[test]
fn a_slash_command_may_follow_leading_whitespace_and_name_a_namespace() {
    assert_eq!(prompt_stats("  /a:b-c x").slash_command.as_deref(), Some("/a:b-c"));
    assert_eq!(prompt_stats("see /fix").slash_command, None);
}

#[test]
fn mentions_need_whitespace_before_the_at_sign_and_urls_stop_at_brackets() {
    let stats = prompt_stats("(see https://a.b/c) <https://d.e> [https://f.g] mail a@b.c and @src/x.py ```a``` ``` ?? /x");
    assert_eq!(stats.at_mentions, strings(&["src/x.py"]));
    assert_eq!(stats.urls, strings(&["https://a.b/c", "https://d.e", "https://f.g"]));
    assert_eq!((stats.code_blocks, stats.questions, stats.chars, stats.words), (1, 2, 90, 12));
}

#[test]
fn credentials_are_named_by_kind_never_by_value() {
    assert_eq!(prompt_stats("ship it, password=hunter2hunter2").secrets, strings(&["password or key assignment"]));
}

#[test]
fn prompt_stats_serialize_without_empty_values() {
    assert_eq!(serde_json::to_value(prompt_stats("hi")).unwrap(), json!({"chars": 2, "words": 1, "lines": 1, "approx_tokens": 1}));
    assert_eq!(serde_json::to_value(prompt_stats("")).unwrap(), json!({}));
}

// --- files a tool touched -------------------------------------------------------------------------

#[test]
fn a_patch_tool_names_its_files_inside_the_patch() {
    assert_eq!(
        files_of(Some("apply_patch"), &json!({"command": PATCH}), &patterns(&[("apply_patch", PATCH_FILES)])),
        strings(&["a.txt", "b.txt"])
    );
}

#[test]
fn a_path_input_wins_over_the_tool_files_pattern() {
    let input = json!({"path": "c.txt", "command": PATCH});
    assert_eq!(files_of(Some("apply_patch"), &input, &patterns(&[("apply_patch", PATCH_FILES)])), strings(&["c.txt"]));
}

#[test]
fn file_path_wins_over_notebook_path_and_path_and_empty_ones_fall_through() {
    let none = IndexMap::new();
    assert_eq!(files_of(Some("Edit"), &json!({"path": "b", "notebook_path": "n", "file_path": "a"}), &none), strings(&["a"]));
    assert_eq!(files_of(Some("Edit"), &json!({"file_path": "", "path": "c"}), &none), strings(&["c"]));
}

#[test]
fn a_tool_without_a_pattern_or_path_touches_no_file() {
    assert_eq!(files_of(Some("Bash"), &json!({"command": PATCH}), &patterns(&[("apply_patch", PATCH_FILES)])), Vec::<String>::new());
    assert_eq!(files_of(None, &json!({"command": PATCH}), &patterns(&[("apply_patch", PATCH_FILES)])), Vec::<String>::new());
}

#[test]
fn an_input_that_is_not_an_object_touches_no_file() {
    assert_eq!(files_of(Some("apply_patch"), &json!("not an object"), &patterns(&[("apply_patch", PATCH_FILES)])), Vec::<String>::new());
}

#[test]
fn a_prefix_pattern_name_applies_to_every_matching_tool() {
    assert_eq!(files_of(Some("patch_v2"), &json!({"command": PATCH}), &patterns(&[("patch*", PATCH_FILES)])), strings(&["a.txt", "b.txt"]));
}

#[test]
fn codex_patches_name_moved_files_too() {
    let patch = "*** Begin Patch\n*** Update File: old.py\n*** Move to: new.py\n*** End Patch";
    assert_eq!(files_of(Some("apply_patch"), &json!({"command": patch}), &spec("codex").tool_files), strings(&["old.py", "new.py"]));
}

// --- one turn ---------------------------------------------------------------------------------------

#[test]
fn a_turn_summary_reports_time_tools_files_answer_and_model_calls() {
    let view = prompt("x").turn(2).prev_at("2026-11-03T09:59:00+00:00").build();
    let events = [
        event("PostToolUse", json!({"tool_name": "Write", "tool_input": {"file_path": "/r/a.py", "content": "x"}, "duration_ms": 1500})),
        tool("Bash", json!({"command": "pytest"})),
        event("api_request", json!({"model": "claude-haiku", "input_tokens": "10", "output_tokens": 5, "cost_usd": 0.01})),
        event("Stop", json!({"last_assistant_message": "Done."})),
    ];
    let summary = turn(&view, &events, Some(&spec("claude-code")));
    assert_eq!((summary.since_previous_prompt_s, summary.response_s, summary.response), (Some(60.0), Some(30.0), Some(json!("Done."))));
    assert_eq!((summary.tool_calls, summary.files_written), (named(&[("Write", 1), ("Bash", 1)]), strings(&["/r/a.py"])));
    assert_eq!((summary.bash_commands, summary.tool_time_s), (strings(&["pytest"]), 1.5));
    assert_eq!(summary.tool_kinds, IndexMap::from([(ToolKind::Write, 1), (ToolKind::Shell, 1)]));
    assert_eq!(summary.usage, usage(1, 10, 5, 0, 0.01));
    assert_eq!(summary.models, named(&[("claude-haiku", 1)]));
    assert_eq!(summary.model_usage, IndexMap::from([(Some("claude-haiku".to_string()), usage(1, 10, 5, 0, 0.01))]));
}

#[test]
fn a_turn_without_a_spec_has_only_other_tools_and_no_model_calls_or_answer() {
    let events = [
        tool("Bash", json!({"command": "ls"})),
        event("api_request", json!({"model": "opus", "input_tokens": 3})),
        event("Stop", json!({"last_assistant_message": "Done."})),
    ];
    let summary = turn(&prompt("x").build(), &events, None);
    assert_eq!(summary.tool_kinds, IndexMap::from([(ToolKind::Other, 1)]));
    assert_eq!((summary.bash_commands.len(), summary.usage.api_requests, summary.response), (0, 0, None));
    assert_eq!(summary.ended_by.as_deref(), Some("Stop"));
}

#[test]
fn the_position_comes_from_the_prompt_view() {
    let first = turn(&prompt("x").turn(1).build(), &[], None);
    assert_eq!((first.turn, first.since_previous_prompt_s), (Some(1), None));
}

#[test]
fn an_unfinished_turn_has_no_ending() {
    let summary = claude_turn(&[tool("Bash", json!({"command": "ls"}))]);
    assert_eq!((summary.response_s, summary.ended_by, summary.error, summary.response), (None, None, None, None));
}

#[test]
fn a_turn_whose_session_ended_before_the_turn_did_was_interrupted() {
    // Claude Code sends no Stop when you stop a turn (Esc, Ctrl+C). Quitting then (Ctrl+C twice,
    // /exit, /clear) sends SessionEnd with the last prompt's id: the turn will never end.
    let events = [
        tool("Bash", json!({"command": "sleep 600"})),
        event_at("SessionEnd", "2026-11-03T10:31:00+00:00", json!({"reason": "prompt_input_exit"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!(
        (summary.ended_by.as_deref(), summary.response_s, summary.error),
        (Some("Interrupt"), None, None),
        "when you stopped it is unknown, so no response time"
    );
}

#[test]
fn a_tool_call_you_interrupted_ends_its_turn_there() {
    let events = [event_at(
        "PostToolUseFailure",
        "2026-11-03T10:00:07+00:00",
        json!({"tool_name": "Bash", "error": "Interrupted by user", "is_interrupt": true}),
    )];
    let summary = claude_turn(&events);
    assert_eq!((summary.ended_by.as_deref(), summary.response_s, summary.error), (Some("Interrupt"), Some(7.0), None));
    assert_eq!(summary.tool_failures, 1, "still a failed tool call");
}

#[test]
fn a_tool_call_that_failed_on_its_own_does_not_end_the_turn() {
    let summary = claude_turn(&[event("PostToolUseFailure", json!({"tool_name": "Bash", "error": "exit 1", "is_interrupt": false}))]);
    assert_eq!(summary.ended_by, None);
}

#[test]
fn a_turn_end_wins_over_a_session_end_in_either_order() {
    // Async hooks can arrive out of order (`claude -p` sends Stop and SessionEnd milliseconds apart).
    let stop = event_at("Stop", "2026-11-03T10:00:10+00:00", json!({"last_assistant_message": "Done."}));
    let end = event_at("SessionEnd", "2026-11-03T10:00:11+00:00", json!({"reason": "other"}));
    for events in [[stop.clone(), end.clone()], [end, stop]] {
        let summary = claude_turn(&events);
        assert_eq!((summary.ended_by.as_deref(), summary.response_s), (Some("Stop"), Some(10.0)));
    }
}

#[test]
fn the_last_turn_ending_event_says_how_and_when_it_ended() {
    let events = [
        event_at("Stop", "2026-11-03T10:00:10+00:00", json!({})),
        event_at("StopFailure", "2026-11-03T10:00:20+00:00", json!({"error": "rate_limit"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!(
        (summary.ended_by.as_deref(), summary.response_s, summary.error),
        (Some("StopFailure"), Some(20.0), Some(json!("rate_limit")))
    );
}

#[test]
fn an_error_message_key_reports_what_went_wrong_at_the_end() {
    let summary = claude_turn(&[event("StopFailure", json!({"error_message": "boom"}))]);
    assert_eq!(summary.error, Some(json!("boom")));
}

#[test]
fn the_answer_is_the_latest_non_empty_one_of_the_turn() {
    let events = [
        event_at("Stop", "2026-11-03T10:00:10+00:00", json!({"last_assistant_message": "First."})),
        event_at("Stop", "2026-11-03T10:00:20+00:00", json!({"last_assistant_message": ""})),
    ];
    assert_eq!(claude_turn(&events).response, Some(json!("First.")));
}

#[test]
fn the_response_time_is_rounded_to_a_tenth_of_a_second() {
    let summary = claude_turn(&[event_at("Stop", "2026-11-03T10:00:01.26+00:00", json!({}))]);
    assert_eq!(summary.response_s, Some(1.3));
}

#[test]
fn z_and_offset_timestamps_measure_the_same_durations() {
    let view = prompt("x").at("2026-11-07T10:00:00.000Z").build();
    let summary = turn(&view, &[event_at("Stop", "2026-11-07T11:00:05+01:00", json!({}))], Some(&spec("claude-code")));
    assert_eq!(summary.response_s, Some(5.0));
}

#[test]
fn failed_tool_calls_are_counted_and_report_their_errors() {
    let events = [
        event("PostToolUseFailure", json!({"tool_name": "Bash", "error": "exit 1"})),
        event("PostToolUseFailure", json!({"tool_name": "MCP:x", "error_message": "connection refused"})),
        event("PostToolUseFailure", json!({"tool_name": "Read"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!((summary.tool_failures, summary.tool_errors), (3, strings(&["exit 1", "connection refused"])));
}

#[test]
fn a_long_tool_error_is_clipped_with_an_ellipsis() {
    let long = format!("{} {}", "x".repeat(279), "y".repeat(30));
    let exact = "z".repeat(280);
    let summary = claude_turn(&[event("PostToolUseFailure", json!({"error": long})), event("PostToolUseFailure", json!({"error": exact}))]);
    assert_eq!(summary.tool_errors, vec![format!("{}…", "x".repeat(279)), exact]);
}

#[test]
fn tool_time_sums_durations_and_ignores_values_that_are_not_numbers() {
    let events = [
        event("PostToolUse", json!({"duration_ms": 1234})),
        event("PostToolUseFailure", json!({"duration_ms": "abc"})),
        event("PostToolUse", json!({"duration_ms": "66"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!((summary.tool_time_s, summary.tool_calls), (1.3, IndexMap::from([(None, 3)])));
}

#[test]
fn files_read_and_written_are_sorted_and_unique() {
    let events = [
        tool("Write", json!({"file_path": "/b"})),
        tool("Edit", json!({"file_path": "/a"})),
        tool("Edit", json!({"file_path": "/b"})),
        tool("Read", json!({"file_path": "/c"})),
        tool("Grep", json!({"path": "/c"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!((summary.files_written, summary.files_read), (strings(&["/a", "/b"]), strings(&["/c"])));
}

#[test]
fn web_tools_report_their_url_else_their_query_and_mcp_tools_are_listed() {
    let events = [
        tool("WebFetch", json!({"url": "https://a.b"})),
        tool("WebSearch", json!({"query": "rust sqlite"})),
        tool("mcp__gh__issue", json!({})),
        tool("mcp__gh__issue", json!({})),
        tool("mcp__db__q", json!({})),
    ];
    let summary = claude_turn(&events);
    assert_eq!(summary.web, vec![Some("https://a.b".to_string()), Some("rust sqlite".to_string())]);
    assert_eq!(summary.mcp_tools, strings(&["mcp__db__q", "mcp__gh__issue"]));
    assert_eq!(
        summary.tool_kind_of,
        IndexMap::from([
            ("WebFetch".to_string(), ToolKind::Web),
            ("WebSearch".to_string(), ToolKind::Web),
            ("mcp__gh__issue".to_string(), ToolKind::Mcp),
            ("mcp__db__q".to_string(), ToolKind::Mcp)
        ])
    );
}

#[test]
fn calls_made_by_subagents_are_counted() {
    let events = [
        event("PostToolUse", json!({"tool_name": "Read", "agent_id": "a1"})),
        event("PostToolUse", json!({"tool_name": "Read", "agent_id": ""})),
    ];
    assert_eq!(claude_turn(&events).subagent_tool_calls, 1);
}

#[test]
fn git_commits_are_shell_commands_that_commit() {
    let commands = [
        "git commit -m x",
        "git -C /r commit",
        "git log --grep commit",
        "git add . && git commit -m x",
        "gitcommit",
        "git -c user.name=x commit",
    ];
    let events: Vec<EventRow> = commands.iter().map(|c| tool("Bash", json!({"command": c}))).collect();
    let summary = claude_turn(&events);
    assert_eq!((summary.git_commits, summary.bash_commands.len()), (4, 6));
}

#[test]
fn commands_of_tools_that_are_not_shells_are_not_shell_commands() {
    assert_eq!(claude_turn(&[tool("Read", json!({"command": "git commit"}))]).git_commits, 0);
}

#[test]
fn agent_tools_report_what_each_subagent_was_asked() {
    let long = "b".repeat(300);
    let events = [
        tool("Agent", json!({"description": "Look around", "prompt": "Find the helpers"})),
        tool("Task", json!({"prompt": long})),
        tool("Agent", json!({"subagent_type": "Explore"})),
    ];
    assert_eq!(
        claude_turn(&events).subagent_tasks,
        vec![
            SubagentTask { task: Some("Look around".into()), brief: Some("Find the helpers".into()) },
            SubagentTask { task: None, brief: Some(format!("{}…", "b".repeat(280))) },
        ]
    );
}

#[test]
fn agentic_activity_is_counted_from_the_turn_events() {
    let events = [
        event("UserPromptExpansion", json!({"command_name": "deploy"})),
        event("SubagentStart", json!({"agent_type": "Explore"})),
        event("SubagentStart", json!({"agent_type": "Explore"})),
        event("TaskCreated", json!({"task_subject": "Write docs"})),
        event("Notification", json!({"notification_type": "idle"})),
        event("PermissionDenied", json!({"tool_name": "Bash"})),
        event("PreCompact", json!({"trigger": "auto"})),
        event("PreCompact", json!({})),
        event("PostModelSwitch", json!({"to_model": "haiku"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!(summary.slash_command.as_deref(), Some("deploy"));
    assert_eq!((summary.subagents, summary.tasks), (named(&[("Explore", 2)]), vec![Some("Write docs".to_string())]));
    assert_eq!((summary.notifications, summary.permission_denied), (named(&[("idle", 1)]), vec![Some("Bash".to_string())]));
    assert_eq!((summary.compactions, summary.compaction_triggers), (2, IndexMap::from([("auto".to_string(), 1)])));
    assert_eq!(summary.model_switches, vec![Some("haiku".to_string())]);
}

#[test]
fn the_first_event_naming_a_command_gives_the_slash_command() {
    let events = [
        event("user_prompt", json!({"command_name": ""})),
        event("user_prompt", json!({"command_name": "effort"})),
        event("UserPromptExpansion", json!({"command_name": "x"})),
    ];
    assert_eq!(claude_turn(&events).slash_command.as_deref(), Some("effort"));
}

#[test]
fn errors_the_tool_reported_keep_name_message_context_and_recoverable() {
    let events = [
        event(
            "ErrorOccurred",
            json!({"error": {"message": "rate limited", "name": "RateLimitError"}, "error_context": "model_call", "recoverable": true}),
        ),
        event("ErrorOccurred", json!({"error": "plain failure", "recoverable": false})),
    ];
    assert_eq!(
        claude_turn(&events).errors,
        vec![
            TurnError {
                name: Some("RateLimitError".into()),
                message: Some("rate limited".into()),
                context: Some(json!("model_call")),
                recoverable: Some(true)
            },
            TurnError { name: None, message: Some("plain failure".into()), context: None, recoverable: None },
        ]
    );
}

#[test]
fn model_calls_are_split_per_model_and_per_query_source() {
    let events = [
        event(
            "api_request",
            json!({"model": "opus", "cost_usd": 1.0, "query_source": "repl_main_thread", "duration_ms": 500, "cache_read_tokens": 7}),
        ),
        event("api_request", json!({"model": "opus", "cost_usd": 0.5, "query_source": "agent:builtin:Explore", "duration_ms": 500})),
        event(
            "api_request",
            json!({"model": "haiku", "cost_usd": 0.25, "query_source": "compact", "duration_ms": 500, "error": "overloaded"}),
        ),
        event("api_error", json!({"model": "opus"})),
        event("tool_decision", json!({"decision": "reject"})),
        event("tool_decision", json!({"decision": "accept"})),
    ];
    let summary = claude_turn(&events);
    assert_eq!(summary.usage, UsageTotals { api_requests: 3, cost_usd: 1.75, cache_read_tokens: 7, ..UsageTotals::default() });
    assert_eq!(summary.models, named(&[("opus", 2), ("haiku", 1)]));
    assert_eq!(
        summary.model_usage[&Some("opus".to_string())],
        UsageTotals { api_requests: 2, cost_usd: 1.5, cache_read_tokens: 7, ..UsageTotals::default() }
    );
    assert_eq!(
        summary.work_usage.keys().cloned().collect::<Vec<_>>(),
        vec![Some("repl_main_thread".into()), Some("agent:builtin:Explore".into()), Some("compact".into())]
    );
    // Claude Code's `usage` (the shared defaults) maps no `error` value, so the call's `error` key is
    // not read: only the `api_error` event counts, as in Python (`c.get("error")` of mapped values).
    assert_eq!((summary.api_time_s, summary.api_errors, summary.tool_rejections), (1.5, 1, 1));
}

#[test]
fn codex_counts_input_tokens_without_the_cached_ones_and_never_below_zero() {
    let codex = spec("codex");
    let payloads = [
        json!({"token_usage": {"input_tokens": 12000, "cached_input_tokens": 9000}}),
        json!({"token_usage": {"input_tokens": 5, "cached_input_tokens": 9}}),
    ];
    let calls = model_calls(&payloads.iter().collect::<Vec<_>>(), &codex.usage);
    assert_eq!(calls.iter().map(|c| (c.input_tokens, c.cache_read_tokens)).collect::<Vec<_>>(), vec![(3000.0, 9000.0), (0.0, 9.0)]);
}

#[test]
fn model_calls_read_the_spec_paths_and_treat_numeric_text_as_numbers() {
    let usage = Usage {
        events: vec!["call".into()],
        values: IndexMap::from([
            (UsageValue::Model, "m".to_string()),
            (UsageValue::InputTokens, "t.in".to_string()),
            (UsageValue::Status, "s".to_string()),
            (UsageValue::Error, "e".to_string()),
        ]),
        input_includes_cached: false,
    };
    let payload = json!({"m": "gpt", "t": {"in": "12"}, "s": "completed", "e": null});
    assert_eq!(
        model_calls(&[&payload], &usage),
        vec![ModelCall {
            model: Some("gpt".into()),
            input_tokens: 12.0,
            status: Some("completed".into()),
            error: false,
            ..ModelCall::default()
        }]
    );
}

#[test]
fn usage_totals_truncate_tokens_and_round_cost_to_four_decimals() {
    let calls = [
        ModelCall { input_tokens: 1.6, cost_usd: 0.123_44, ..ModelCall::default() },
        ModelCall { input_tokens: 1.6, cost_usd: 0.000_02, ..ModelCall::default() },
    ];
    assert_eq!(usage_of(&calls), UsageTotals { api_requests: 2, input_tokens: 3, cost_usd: 0.1235, ..UsageTotals::default() });
}

#[test]
fn work_kinds_tell_the_main_thread_subagents_and_system_calls_apart() {
    use WorkKind::*;
    let kinds = [None, Some("repl_main_thread"), Some("sdk"), Some("agent:builtin:Explore"), Some("compact"), Some("")].map(work_kind);
    assert_eq!(kinds, [Main, Main, Main, Subagent, System, System]);
    assert_eq!([Main.as_str(), Subagent.as_str(), System.as_str()], ["main", "subagent", "system"]);
}

#[test]
fn a_turn_summary_serializes_compacted_with_usage_at_the_top_level() {
    let view = prompt("x").turn(2).prev_at("2026-11-03T09:59:00+00:00").build();
    let events = [tool("Write", json!({"file_path": "/r/a.py"})), event("Stop", json!({"last_assistant_message": "Done."}))];
    let summary = turn(&view, &events, Some(&spec("claude-code")));
    assert_eq!(
        serde_json::to_value(summary).unwrap(),
        json!({
            "turn": 2, "since_previous_prompt_s": 60.0, "response_s": 30.0, "ended_by": "Stop", "response": "Done.",
            "tool_calls": {"Write": 1}, "files_written": ["/r/a.py"], "tool_kinds": {"write": 1}, "tool_kind_of": {"Write": "write"}
        })
    );
}

#[test]
fn model_usage_serializes_per_model_with_every_measure() {
    let summary = claude_turn(&[event("api_request", json!({"model": "opus", "input_tokens": 10}))]);
    let value = serde_json::to_value(summary).unwrap();
    assert_eq!(
        value["model_usage"],
        json!({"opus": {"api_requests": 1, "input_tokens": 10, "output_tokens": 0, "cache_read_tokens": 0, "cache_creation_tokens": 0, "cost_usd": 0.0}})
    );
    assert_eq!((value["api_requests"].clone(), value["input_tokens"].clone()), (json!(1), json!(10)));
}

#[test]
fn a_tool_without_a_name_serializes_under_null() {
    let value = serde_json::to_value(claude_turn(&[event("PostToolUse", json!({}))])).unwrap();
    assert_eq!(value["tool_calls"], json!({"null": 1}));
}

// --- connector fixtures ----------------------------------------------------------------------------

#[test]
fn copilot_tool_calls_get_their_kinds_files_and_commands() {
    let copilot = spec("copilot-cli");
    let mut events: Vec<EventRow> = replay(&copilot, "copilot-cli")
        .all_events()
        .into_iter()
        .filter(|e| derivations::TOOL_RESULTS.contains(&e.event.as_deref().unwrap_or("")))
        .collect();
    events.push(event_at(
        "PostToolUse",
        "2026-11-07T10:00:20.000Z",
        json!({"tool_name": "bash", "tool_input": {"command": "git commit -m x"}}),
    ));
    let summary = turn(&prompt("x").at("2026-11-07T10:00:00.000Z").build(), &events, Some(&copilot));
    assert_eq!(
        summary.tool_kind_of,
        IndexMap::from([
            ("bash".to_string(), ToolKind::Shell),
            ("edit".to_string(), ToolKind::Write),
            ("view".to_string(), ToolKind::Read)
        ])
    );
    assert_eq!((summary.files_written, summary.files_read), (strings(&["client.py"]), strings(&["README.md"])));
    assert_eq!((summary.bash_commands, summary.git_commits), (strings(&["ls src", "git commit -m x"]), 1));
}

#[test]
fn codex_turns_get_files_from_patches_subagent_tasks_and_the_answer() {
    let codex = spec("codex");
    let fixtures = replay(&codex, "codex");
    let summary = turn(fixtures.prompt("02_user_prompt_submit"), &fixtures.turn("02_user_prompt_submit"), Some(&codex));
    assert_eq!(
        summary.tool_kind_of,
        IndexMap::from([
            ("Bash".to_string(), ToolKind::Shell),
            ("apply_patch".to_string(), ToolKind::Write),
            ("collaborationspawn_agent".to_string(), ToolKind::Agent)
        ])
    );
    assert_eq!(summary.files_written, strings(&["/work/app/upload.py"]));
    assert_eq!(summary.bash_commands, strings(&["pytest -q tests/test_upload.py", "cat missing.txt"]));
    assert_eq!(
        summary.subagent_tasks,
        vec![SubagentTask { task: Some("find_send".into()), brief: Some("Find where the upload client sends chunks".into()) }]
    );
    assert!(summary.response.as_ref().and_then(Json::as_str).is_some_and(|r| r.starts_with("Added a retry")), "{:?}", summary.response);
    assert_eq!(summary.ended_by.as_deref(), Some("Stop"));
}

#[test]
fn codex_model_requests_report_tokens_without_cached_input_and_no_cost() {
    let codex = spec("codex");
    let fixtures = replay(&codex, "codex");
    let summary = turn(fixtures.prompt("02_user_prompt_submit"), &fixtures.turn("02_user_prompt_submit"), Some(&codex));
    let expected = UsageTotals {
        api_requests: 1,
        input_tokens: 3000,
        output_tokens: 300,
        cache_read_tokens: 9000,
        cache_creation_tokens: 0,
        cost_usd: 0.0,
    };
    assert_eq!(summary.model_usage, IndexMap::from([(Some("gpt-5-codex".to_string()), expected.clone())]));
    assert_eq!((summary.usage, summary.reasoning_tokens), (expected, 120));
    assert_eq!(summary.model_call_status, IndexMap::from([("completed".to_string(), 1)]));
}

#[test]
fn codex_interrupted_turn_with_a_failed_model_call() {
    let codex = spec("codex");
    let fixtures = replay(&codex, "codex");
    let view = prompt("x").at("2026-11-07T10:00:00+00:00").build();
    let summary = turn(&view, &all_at(fixtures.turn("09_user_prompt_submit_stdin"), "2026-11-07T10:00:05+00:00"), Some(&codex));
    assert_eq!((summary.ended_by.as_deref(), summary.response_s), (Some("Interrupt"), Some(5.0)));
    assert_eq!((summary.usage.api_requests, summary.api_errors, summary.usage.input_tokens), (1, 1, 0));
    assert_eq!((summary.model_call_status, summary.compactions), (IndexMap::from([("failed".to_string(), 1)]), 1));
}

#[test]
fn cursor_turns_take_the_answer_from_the_agent_response() {
    let cursor = spec("cursor");
    let fixtures = replay(&cursor, "cursor");
    let summary = turn(fixtures.prompt("02_before_submit_prompt"), &fixtures.turn("02_before_submit_prompt"), Some(&cursor));
    assert_eq!((summary.response, summary.ended_by.as_deref()), (Some(json!("Added an optional coupon field and a test.")), Some("Stop")));
    assert_eq!((summary.tool_failures, summary.tool_errors), (1, strings(&["connection refused"])));
}

#[test]
fn cursor_aborted_and_failed_stops_end_turns_as_interrupt_and_stop_failure() {
    let cursor = spec("cursor");
    let fixtures = replay(&cursor, "cursor");
    let aborted = turn(fixtures.prompt("10_before_submit_prompt_slash"), &fixtures.turn("10_before_submit_prompt_slash"), Some(&cursor));
    let failed =
        turn(fixtures.prompt("12_before_submit_prompt_one_folder"), &fixtures.turn("12_before_submit_prompt_one_folder"), Some(&cursor));
    assert_eq!((aborted.ended_by.as_deref(), failed.ended_by.as_deref()), (Some("Interrupt"), Some("StopFailure")));
}

#[test]
fn copilot_errors_during_the_turn_are_reported() {
    let copilot = spec("copilot-cli");
    let fixtures = replay(&copilot, "copilot-cli");
    let summary = turn(fixtures.prompt("09_user_prompt_submit_subagent"), &fixtures.turn("09_user_prompt_submit_subagent"), Some(&copilot));
    assert_eq!(
        summary.errors,
        vec![TurnError {
            name: Some("RateLimitError".into()),
            message: Some("rate limited".into()),
            context: Some(json!("model_call")),
            recoverable: Some(true)
        }]
    );
}

// --- the session -------------------------------------------------------------------------------------

#[test]
fn session_starts_list_how_it_began_and_each_resume() {
    let codex = spec("codex");
    let summary = session(&replay(&codex, "codex").all_events());
    assert_eq!((summary.starts, summary.ended.as_deref()), (strings(&["startup", "resume"]), Some("other")));
}

#[test]
fn a_session_counts_its_events_and_when_they_came() {
    let events = [
        event_at("SessionStart", "2026-11-03T10:00:00+00:00", json!({})),
        event_at("Stop", "2026-11-03T10:05:00+00:00", json!({})),
        event_at("Stop", "2026-11-03T10:09:00+00:00", json!({})),
    ];
    let summary = session(&events);
    assert_eq!(summary.events, named(&[("SessionStart", 1), ("Stop", 2)]));
    assert_eq!(
        (summary.first_event_at.as_deref(), summary.last_event_at.as_deref()),
        (Some("2026-11-03T10:00:00+00:00"), Some("2026-11-03T10:09:00+00:00"))
    );
}

#[test]
fn an_empty_session_has_nothing_to_report() {
    assert_eq!(serde_json::to_value(session(&[])).unwrap(), json!({}));
}

#[test]
fn the_last_session_end_gives_the_reason() {
    let events = [event("SessionEnd", json!({"reason": "clear"})), event("SessionEnd", json!({"reason": "logout"}))];
    assert_eq!(session(&events).ended.as_deref(), Some("logout"));
}

#[test]
fn instructions_directory_and_config_changes_are_listed() {
    let events = [
        event("InstructionsLoaded", json!({"file_path": "/r/CLAUDE.md"})),
        event("InstructionsLoaded", json!({"file_path": "/a/CLAUDE.md"})),
        event("InstructionsLoaded", json!({"file_path": "/r/CLAUDE.md"})),
        event("InstructionsLoaded", json!({})),
        event("CwdChanged", json!({"new_cwd": "/r/sub"})),
        event("ConfigChange", json!({"file_path": "/r/.claude/settings.json", "source": "project"})),
        event("ConfigChange", json!({"source": "user"})),
    ];
    let summary = session(&events);
    assert_eq!(summary.instructions_loaded, strings(&["/a/CLAUDE.md", "/r/CLAUDE.md"]));
    assert_eq!(summary.cwd_changes, vec![Some("/r/sub".to_string())]);
    assert_eq!(summary.config_changes, vec![Some("/r/.claude/settings.json".to_string()), Some("user".to_string())]);
}

#[test]
fn cursor_sessions_report_their_mode_and_whether_they_ran_in_the_background() {
    let cursor = spec("cursor");
    let summary = session(&replay(&cursor, "cursor").all_events());
    assert_eq!((summary.modes, summary.background), (strings(&["agent"]), false));
    let background = session(&[event("SessionStart", json!({"is_background_agent": true}))]);
    assert!(background.background);
}

#[test]
fn finished_subagents_report_type_stop_reason_and_last_message_by_any_tool_names() {
    let events = [
        event(
            "SubagentStop",
            json!({"agent_id": "agent_0001", "agent_type": "default", "last_assistant_message": "send() is in upload.py, line 42."}),
        ),
        event("SubagentStop", json!({"subagent_type": "explore", "status": "completed", "summary": "Tests are in src/checkout.test.ts"})),
        event(
            "SubagentStop",
            json!({"agent_id": "sub-1", "agent_type": "explore", "stop_reason": "end_turn", "last_assistant_message": "Found two callers."}),
        ),
    ];
    assert_eq!(
        session(&events).subagent_results,
        vec![
            SubagentResult {
                agent_id: Some("agent_0001".into()),
                agent_type: Some("default".into()),
                stop_reason: None,
                answer: Some("send() is in upload.py, line 42.".into())
            },
            SubagentResult {
                agent_id: None,
                agent_type: Some("explore".into()),
                stop_reason: Some("completed".into()),
                answer: Some("Tests are in src/checkout.test.ts".into())
            },
            SubagentResult {
                agent_id: Some("sub-1".into()),
                agent_type: Some("explore".into()),
                stop_reason: Some("end_turn".into()),
                answer: Some("Found two callers.".into())
            },
        ]
    );
}

#[test]
fn telemetry_metrics_are_totalled_per_name_and_type() {
    let events = [
        event("claude_code.lines_of_code.count", json!({"type": "added", "value": 3})),
        event("claude_code.lines_of_code.count", json!({"type": "added", "value": "4"})),
        event("claude_code.commit.count", json!({"value": 1})),
        event("claude_code.cost.usage", json!({"value": 0.000_011})),
        event("claude_code.cost.usage", json!({"value": 0.000_011})),
        event("api_request", json!({"value": 9})),
    ];
    assert_eq!(
        session(&events).metric_totals,
        IndexMap::from([
            ("claude_code.lines_of_code.count[added]".to_string(), 7.0),
            ("claude_code.commit.count".to_string(), 1.0),
            ("claude_code.cost.usage".to_string(), 0.0)
        ])
    );
}

#[test]
fn the_environment_is_the_resource_of_the_latest_telemetry_event() {
    let events = [
        event("api_request", json!({"session.id": "s", "service.name": "claude-code", "app.version": "1.0", "model": "opus"})),
        event("api_request", json!({"session.id": "s", "service.name": "claude-code", "terminal.type": "iTerm"})),
        event("Stop", json!({"app.version": "hook"})),
    ];
    assert_eq!(session(&events).environment, obj(json!({"service.name": "claude-code", "terminal.type": "iTerm"})));
}

#[test]
fn a_metric_data_point_is_named_with_its_type() {
    assert_eq!(
        metric(&event("claude_code.token.usage", json!({"type": "input", "value": 5}))),
        Some(("claude_code.token.usage[input]".to_string(), 5.0))
    );
    assert_eq!(metric(&event("claude_code.commit.count", json!({"value": 2}))), Some(("claude_code.commit.count".to_string(), 2.0)));
}

#[test]
fn events_that_are_not_metric_data_points_have_no_metric() {
    let nameless = EventRow { event: None, ..event("x", json!({"value": 1})) };
    assert_eq!(
        [metric(&event("api_request", json!({"value": 1}))), metric(&event("claude_code.commit.count", json!({}))), metric(&nameless)],
        [None, None, None]
    );
}

#[test]
fn telemetry_slash_commands_get_exactly_one_slash_and_their_source() {
    assert_eq!(
        command(&event("user_prompt", json!({"command_name": "model", "command_source": "builtin"}))),
        Some(("/model".to_string(), Some("builtin".to_string())))
    );
    assert_eq!(command(&event("user_prompt", json!({"command_name": "//review"}))), Some(("/review".to_string(), None)));
}

#[test]
fn prompts_without_a_command_and_other_events_are_not_commands() {
    assert_eq!(
        [command(&event("user_prompt", json!({}))), command(&event("UserPromptExpansion", json!({"command_name": "x"})))],
        [None, None]
    );
}

#[test]
fn a_session_summary_serializes_compacted() {
    let value =
        serde_json::to_value(session(&[event_at("SessionStart", "2026-11-03T10:00:00+00:00", json!({"source": "startup"}))])).unwrap();
    assert_eq!(
        value,
        json!({"events": {"SessionStart": 1}, "first_event_at": "2026-11-03T10:00:00+00:00", "last_event_at": "2026-11-03T10:00:00+00:00", "starts": ["startup"]})
    );
}

// --- derive -------------------------------------------------------------------------------------------

#[test]
fn derive_adds_origin_typed_text_surface_stats_and_summary() {
    let view = prompt("<ide_selection>x</ide_selection>\nRename x").build();
    let derived = derive(
        &view,
        &[event("Stop", json!({})), common::telemetry(json!({"app.entrypoint": "claude-vscode"}))],
        Some(&spec("claude-code")),
    );
    assert_eq!((derived.origin(), derived.classified.typed_text.as_deref()), (gitprompt_core::spec::Origin::Typed, Some("Rename x")));
    assert_eq!((derived.surface.as_deref(), derived.stats.chars, derived.summary.ended_by.as_deref()), (Some("vscode"), 41, Some("Stop")));
    assert_eq!(derived.prompt, view);
}

#[test]
fn derive_of_a_prompt_without_text_has_empty_stats() {
    let derived = derive(&prompt("x").no_text().build(), &[], Some(&spec("claude-code")));
    assert_eq!((derived.origin(), derived.stats), (gitprompt_core::spec::Origin::Unknown, PromptStats::default()));
}

#[test]
fn derived_values_serialize_next_to_the_stored_row() {
    let value = serde_json::to_value(derive(&prompt("hi").build(), &[], Some(&spec("claude-code")))).unwrap();
    assert_eq!(
        (value["id"].clone(), value["origin"].clone(), value["typed_text"].clone(), value["surface"].clone()),
        (json!(1), json!("typed"), json!("hi"), json!(null))
    );
    assert_eq!((value["injected"].clone(), value["summary"].clone()), (json!([]), json!({"turn": 1})));
}

#[test]
fn every_derived_value_is_documented_for_the_fields_page() {
    let docs = derivations::docs();
    let expected = [
        "origin",
        "typed_text",
        "injected",
        "surface",
        "stats.secrets",
        "summary.turn",
        "session.metric_totals",
        "facts.tokens",
        "commits / diffstat",
        "context.git_*",
        "context.tool_version",
    ];
    let missing: Vec<&str> = expected.into_iter().filter(|k| !docs.contains_key(*k)).collect();
    assert_eq!(missing, Vec::<&str>::new());
    assert!(docs.values().all(|meaning| !meaning.trim().is_empty()));
}
