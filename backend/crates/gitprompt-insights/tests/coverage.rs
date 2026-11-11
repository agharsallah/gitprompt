//! Field coverage (coverage.rs): every key a connector's fixture payloads carry is mapped, derived,
//! shown or ignored with a reason (ADR-0021). Port of `tests/sources/test_field_coverage.py` over
//! testdata/fixtures, replayed without the capture pipeline (tests/common `replay`).
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

use std::collections::{BTreeMap, BTreeSet};

use pretty_assertions::assert_eq;
use serde_json::json;

use common::{cases, replay, spec, specs};
use gitprompt_insights::coverage::{declared, derived_reads, uncovered};

fn set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(std::string::ToString::to_string).collect()
}

/// fixture name -> its payload keys neither declared by the spec nor read by the derivations.
fn missing(tool: &str) -> BTreeMap<String, Vec<String>> {
    let spec = spec(tool);
    let fixtures = replay(&spec, tool);
    let (static_keys, reads) = (declared(&spec), derived_reads(&spec, &fixtures.all_prompts(), &fixtures.all_events()));
    cases(tool)
        .into_iter()
        .map(|case| (case.name, uncovered(&case.payload, &static_keys, &reads)))
        .filter(|(_, keys)| !keys.is_empty())
        .collect()
}

#[test]
fn every_claude_code_fixture_key_is_used_or_ignored_with_a_reason() {
    assert_eq!(missing("claude-code"), BTreeMap::new());
}

#[test]
fn every_codex_fixture_key_is_used_or_ignored_with_a_reason() {
    assert_eq!(missing("codex"), BTreeMap::new());
}

#[test]
fn every_copilot_fixture_key_is_used_or_ignored_with_a_reason() {
    assert_eq!(missing("copilot-cli"), BTreeMap::new());
}

#[test]
fn every_cursor_fixture_key_is_used_or_ignored_with_a_reason() {
    assert_eq!(missing("cursor"), BTreeMap::new());
}

#[test]
fn an_unknown_key_fails_coverage() {
    let codex = spec("codex");
    let fixtures = replay(&codex, "codex");
    let reads = derived_reads(&codex, &fixtures.all_prompts(), &fixtures.all_events());
    let mut payload = cases("codex")[1].payload.clone();
    payload["brand_new_field"] = json!(1);
    payload["token_usage"] = json!({"input_tokens": 1, "new_kind": 2});
    assert_eq!(uncovered(&payload, &declared(&codex), &reads), vec!["brand_new_field".to_string(), "token_usage.new_kind".to_string()]);
}

#[test]
fn a_spec_declares_its_mapped_model_call_answer_rule_shown_ignored_and_discarded_keys() {
    let cursor = declared(&spec("cursor"));
    let codex = declared(&spec("codex"));
    let claude = declared(&spec("claude-code"));
    assert!(set(&["workspace_roots[]", "cursor_version", "text", "user_email", "model_id"]).is_subset(&cursor), "{cursor:?}");
    assert!(set(&["turn_id", "token_usage.cached_input_tokens", "token_usage.total_tokens", "status"]).is_subset(&codex), "{codex:?}");
    assert!(set(&["app.entrypoint", "service.name", "last_assistant_message", "tool_response"]).is_subset(&claude), "{claude:?}");
}

#[test]
fn a_declared_key_covers_what_is_under_it() {
    let payload = json!({"tool_response": {"stdout": "x", "files": [{"path": "a"}]}});
    assert_eq!(uncovered(&payload, &set(&["tool_response"]), &BTreeSet::new()), Vec::<String>::new());
}

#[test]
fn a_key_read_as_a_whole_covers_what_is_under_it() {
    let payload = json!({"tool_input": {"command": "ls", "timeout": 3}});
    assert_eq!(uncovered(&payload, &BTreeSet::new(), &set(&["tool_input"])), Vec::<String>::new());
}

#[test]
fn a_key_read_into_covers_only_what_was_read() {
    let payload = json!({"token_usage": {"input_tokens": 1, "new_kind": 2}});
    assert_eq!(
        uncovered(&payload, &BTreeSet::new(), &set(&["token_usage", "token_usage.input_tokens"])),
        vec!["token_usage.new_kind".to_string()]
    );
}

#[test]
fn first_list_items_and_any_list_item_name_the_same_key() {
    let payload = json!({"workspace_roots": ["/a", "/b"], "other": 1});
    assert_eq!(uncovered(&payload, &set(&["workspace_roots[]"]), &BTreeSet::new()), vec!["other".to_string()]);
}

#[test]
fn the_derivations_read_tool_inputs_answers_and_model_call_tokens() {
    let codex = spec("codex");
    let fixtures = replay(&codex, "codex");
    let reads = derived_reads(&codex, &fixtures.all_prompts(), &fixtures.all_events());
    assert!(
        set(&["tool_name", "tool_input.command", "last_assistant_message", "token_usage.input_tokens", "source", "reason"])
            .is_subset(&reads),
        "{reads:?}"
    );
}

#[test]
fn ignored_and_shown_keys_have_a_reason() {
    let blank: Vec<(String, String)> = specs()
        .values()
        .flat_map(|s| {
            s.ignored
                .iter()
                .chain(s.shown.iter())
                .filter(|(_, why)| why.trim().is_empty())
                .map(|(k, _)| (s.id.clone(), k.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(blank, vec![]);
}
