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
    findings(&derived, events, &[])
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

mod hiding {
    use super::{EventRow, KEY, Place, derive, event, findings, json, prompt, seen, spec};
    use gitprompt_insights::leaks::{private_terms, redactor};
    use pretty_assertions::assert_eq;

    fn derived(text: &str, events: &[EventRow]) -> gitprompt_insights::derivations::Derived {
        derive(&prompt(text).id(7).build(), events, Some(&spec("claude-code")))
    }

    #[test]
    fn a_redactor_replaces_the_value_and_never_shows_it() {
        let d = derived(&format!("use {KEY} now"), &[]);
        let id = findings(&d, &[], &[])[0].id.clone();
        let hide = redactor(&d, &[], &id, &[]).unwrap();
        assert_eq!(
            hide.apply(&json!({"prompt": format!("use {KEY} now"), "n": 1})),
            json!({"prompt": "use [hidden OpenAI API key] now", "n": 1})
        );
        assert!(!format!("{hide:?}").contains("q8Zt2Lw9XkP4vR7mN3cB"));
        assert!(redactor(&d, &[], "0000000000000000", &[]).is_none(), "another turn's finding");
    }

    #[test]
    fn a_word_of_your_own_list_is_found_masked_and_hidden_with_that_list() {
        let terms = private_terms(&[" Project Falcon ".to_owned(), String::new()]);
        let d = derived("ship project falcon today", &[]);
        let found = findings(&d, &[], &terms);
        assert_eq!(seen(&found), [("private term", "pro…lcon", Place::Prompt)]);
        assert_eq!(findings(&d, &[], &[]), [], "without the list, nothing");
        assert!(redactor(&d, &[], &found[0].id, &[]).is_none(), "the value is found again with the list only");
        let hide = redactor(&d, &[], &found[0].id, &terms).unwrap();
        assert_eq!(hide.apply(&json!("ship project falcon today")), json!("ship [hidden private term] today"));
    }

    #[test]
    fn hiding_a_sensitive_file_hides_what_reading_it_returned() {
        let read = event(
            "PostToolUse",
            json!({"tool_name": "Read", "tool_input": {"file_path": "/app/.env"}, "tool_response": {"content": "A=1"}}),
        );
        let d = derived("check the config", std::slice::from_ref(&read));
        let id = findings(&d, std::slice::from_ref(&read), &[])[0].id.clone();
        let hide = redactor(&d, std::slice::from_ref(&read), &id, &[]).unwrap();
        let other = json!({"tool_input": {"file_path": "/app/main.rs"}, "tool_response": "fn main"});
        assert_eq!(hide.apply(&read.raw)["tool_response"], "[hidden environment file]");
        assert_eq!(hide.apply(&other), other);
    }
}

mod board {
    use std::collections::BTreeSet;

    use gitprompt_insights::leaks::{LeakDecision, allow_keys, board, value_key};
    use gitprompt_store::leaks::{LeakState, StoredLeak};
    use pretty_assertions::assert_eq;

    fn stored(finding: &str, value: &str) -> StoredLeak {
        StoredLeak {
            finding: finding.into(),
            value_key: value_key("email address", value),
            kind: "email address".into(),
            place: "prompt".into(),
            masked: "m…@dupont.io".into(),
            prompt: 1,
            received_at: "2026-11-18T10:00:00.000000+00:00".into(),
            session: None,
        }
    }

    fn state(leak: &StoredLeak, decision: LeakDecision) -> LeakState {
        LeakState {
            finding: leak.finding.clone(),
            value_key: leak.value_key.clone(),
            kind: leak.kind.clone(),
            masked: leak.masked.clone(),
            place: leak.place.clone(),
            prompt: leak.prompt,
            state: decision.as_str().into(),
            at: "2026-11-18T11:00:00.000000+00:00".into(),
        }
    }

    #[test]
    fn decisions_and_the_allow_list_sort_findings() {
        let (a, a_again, b) = (stored("a", "marie@dupont.io"), stored("a2", "Marie@dupont.io "), stored("b", "paul@dupont.io"));
        let gone = stored("h", "zoe@dupont.io");
        let states = [state(&a, LeakDecision::NotSensitive), state(&gone, LeakDecision::Hidden)];
        let out = board(vec![a, a_again, b], &states, &BTreeSet::new(), 3);
        let names = |list: &[gitprompt_insights::leaks::LeakOut]| list.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            (names(&out.open), names(&out.dismissed), names(&out.hidden)),
            (vec!["b".to_owned()], vec!["a".to_owned(), "a2".to_owned()], vec!["h".to_owned()])
        );
        let allowed = board(vec![stored("b", "paul@dupont.io")], &[], &allow_keys(&["Paul@Dupont.io".to_owned()]), 1);
        assert_eq!(allowed.open, []);
    }
}
