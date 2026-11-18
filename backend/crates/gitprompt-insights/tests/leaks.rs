//! Possible leaks of a turn: where each one sits, masked, with ids that stay the same when the turn
//! is read again. Every value here is synthetic.
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

use common::{event, prompt, spec, tool};
use gitprompt_insights::derivations::derive;
use gitprompt_insights::leaks::{Finding, PER_TURN, Place, findings};
use gitprompt_store::EventRow;

const KEY: &str = "sk-proj-q8Zt2Lw9XkP4vR7mN3cBy6Hd";

fn turn(text: &str, events: &[EventRow]) -> Vec<Finding> {
    let derived = derive(&prompt(text).id(7).build(), events, Some(&spec("claude-code")));
    findings(&derived, events)
}

fn seen(findings: &[Finding]) -> Vec<(&str, &str, Place)> {
    findings.iter().map(|f| (f.kind.as_str(), f.masked.as_str(), f.place)).collect()
}

#[test]
fn leaks_are_found_in_the_prompt_the_answer_and_the_tools() {
    let events = [
        tool("Read", json!({"file_path": "/home/me/app/.env"})),
        event(
            "PostToolUse",
            json!({"tool_name": "Bash", "tool_input": {"command": "cat notes"}, "tool_response": {"stdout": "mail marie.curie@dupont.io"}}),
        ),
        event("Stop", json!({"last_assistant_message": "Call +49 30 1234567 about it"})),
    ];
    let found = turn(&format!("use {KEY} to call the API"), &events);
    assert_eq!(
        seen(&found),
        [
            ("OpenAI API key", "sk-…y6Hd", Place::Prompt),
            ("phone number", "+49…567", Place::Answer),
            ("environment file", ".env", Place::ToolInput),
            ("email address", "m…@dupont.io", Place::ToolOutput),
        ]
    );
}

#[test]
fn the_clear_value_is_in_no_field_of_a_finding() {
    let found = turn(&format!("use {KEY}"), &[]);
    let debug = format!("{found:?}");
    assert!(!debug.contains("q8Zt2Lw9XkP4vR7mN3cB"), "{debug}");
}

#[test]
fn ids_are_stable_and_a_value_has_one_key_wherever_it_is() {
    let first = turn(&format!("use {KEY}"), &[]);
    assert_eq!(first, turn(&format!("use {KEY}"), &[]), "the same turn read again");
    let answer = turn("hi", &[event("Stop", json!({"last_assistant_message": format!("set {KEY}")}))]);
    assert_eq!(first[0].value_key, answer[0].value_key, "same value");
    assert_ne!(first[0].id, answer[0].id, "another place");
    assert_eq!(first[0].id.len(), 16);
}

#[test]
fn a_value_found_twice_in_one_place_is_one_finding_and_a_turn_keeps_at_most_twenty() {
    assert_eq!(turn(&format!("{KEY} and again {KEY}"), &[]).len(), 1);
    let many = (0..40).map(|n| format!("person{n}@dupont.io")).collect::<Vec<_>>().join("\n");
    assert_eq!(turn(&many, &[]).len(), PER_TURN);
}

#[test]
fn an_ordinary_coding_turn_has_no_finding() {
    let events = [
        tool("Read", json!({"file_path": "/repo/src/main.rs"})),
        event(
            "PostToolUse",
            json!({"tool_name": "Bash", "tool_input": {"command": "git log -1"}, "tool_response": {"stdout": "commit 9fceb02d0ae598e95dc970b74767f19372d61af8\nAuthor: Jane <jane@acme.io>\nDate: Tue Nov 18 10:42:11 2026 +0100"}}),
        ),
        event("Stop", json!({"last_assistant_message": "Done: bumped lodash@4.17.21 and set OPENAI_API_KEY=os.environ[\"KEY\"]"})),
    ];
    assert_eq!(turn("update the deps, see .env.example", &events), []);
}

#[test]
fn a_phone_number_counts_where_it_was_sent_not_in_a_page_a_tool_read() {
    let page = event(
        "PostToolUse",
        json!({"tool_name": "WebFetch", "tool_input": {"url": "https://shop.io"}, "tool_response": "Call us: +49 30 1234567"}),
    );
    assert_eq!(turn("what does the page say?", &[page]), []);
    let typed = turn("my number is +49 30 1234567", &[]);
    assert_eq!(seen(&typed), [("phone number", "+49…567", Place::Prompt)]);
}
