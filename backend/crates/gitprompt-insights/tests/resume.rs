//! The resume action of a session (derivations `resume`): present only when the tool's spec has a
//! template, with the folder the session started in, and serialized only when present.
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

use pretty_assertions::assert_eq;
use serde_json::json;

use common::{event, obj, spec};
use gitprompt_insights::derivations::{Resume, resume, session};
use gitprompt_store::EventRow;

fn in_folder(folder: &str) -> EventRow {
    EventRow { fields: obj(json!({"cwd": folder})), ..event("PostToolUse", json!({})) }
}

#[test]
fn a_tool_with_a_template_gets_its_command_and_the_folder_the_session_started_in() {
    let events = [event("Stop", json!({})), in_folder("/work/app"), in_folder("/work/app/sub")];
    let found = resume(&spec("claude-code"), "abc-123", &events, Some("/elsewhere"));
    assert_eq!(found, Some(Resume { command: "claude --resume abc-123".into(), cwd: Some("/work/app".into()) }));
}

#[test]
fn the_prompts_folder_is_used_when_no_event_has_one() {
    let found = resume(&spec("codex"), "abc", &[event("Stop", json!({}))], Some("/work/app"));
    assert_eq!(found, Some(Resume { command: "codex resume abc".into(), cwd: Some("/work/app".into()) }));
}

#[test]
fn a_session_with_no_folder_still_resumes_when_the_template_does_not_need_one() {
    assert_eq!(resume(&spec("copilot-cli"), "abc", &[], None), Some(Resume { command: "copilot --resume=abc".into(), cwd: None }));
}

#[test]
fn a_tool_without_a_template_has_no_action() {
    assert_eq!(resume(&spec("cursor"), "abc", &[in_folder("/work")], Some("/work")), None);
}

#[test]
fn the_summary_leaves_resume_out_unless_it_has_one() {
    let summary = session(&[event("Stop", json!({}))]);
    assert!(!serde_json::to_value(&summary).unwrap().as_object().unwrap().contains_key("resume"));

    let mut with = summary;
    with.resume = resume(&spec("claude-code"), "abc", &[], None);
    assert_eq!(serde_json::to_value(&with).unwrap()["resume"], json!({"command": "claude --resume abc"}));
}
