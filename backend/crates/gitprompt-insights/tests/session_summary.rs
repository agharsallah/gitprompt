//! The session summary (derivations.rs `session`): how a session began and ended, its events,
//! telemetry metrics and slash commands. Port of the session parts of `tests/insights/test_insights.py`
//! and the session behaviour the API tests assert.
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

use common::{event, event_at, named, obj, replay, spec, strings};
use gitprompt_insights::derivations::{SubagentResult, command, metric, session};
use gitprompt_store::EventRow;

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
