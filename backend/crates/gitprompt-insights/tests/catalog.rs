//! The Fields page (catalog.rs): fields seen in stored payloads next to the fields each tool documents
//! and the ones it deliberately leaves unused. Port of the catalog parts of
//! `tests/sources/test_field_coverage.py` and `tests/api/test_api.py`.
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
use serde_json::{Value, json};

use common::specs;
use gitprompt_insights::catalog::{Catalog, Field, SAMPLE_CHARS, build_catalog};
use gitprompt_insights::derivations;
use gitprompt_store::store::Sample;

fn sample(source: &str, event: &str, data: Value) -> Sample {
    Sample { source: source.into(), event: event.into(), data }
}

fn catalog_of(samples: &[Sample]) -> Catalog {
    let specs = specs();
    build_catalog(samples, specs.values().map(std::convert::AsRef::as_ref))
}

fn field<'a>(catalog: &'a Catalog, source: &str, event: &str, path: &str) -> &'a Field {
    catalog
        .fields
        .iter()
        .find(|f| f.source == source && f.event == event && f.path == path)
        .unwrap_or_else(|| panic!("no field {source} {event} {path}"))
}

#[test]
fn the_fields_page_says_why_a_field_is_not_used() {
    let catalog = catalog_of(&[sample(
        "codex",
        "PostModelRequest",
        json!({"model": "gpt-6-luna", "token_usage": {"input_tokens": 1, "total_tokens": 2}}),
    )]);
    let reason = specs()["codex"].ignored["token_usage.total_tokens"].clone();
    assert_eq!(field(&catalog, "codex", "PostModelRequest", "token_usage.total_tokens").unused, Some(reason));
    assert_eq!(field(&catalog, "codex", "PostModelRequest", "token_usage.input_tokens").unused, None);
    assert_eq!(field(&catalog, "codex", "PostModelRequest", "model").unused, None);
}

#[test]
fn a_field_counts_its_values_and_keeps_the_last_non_empty_sample_cut_short() {
    let catalog = catalog_of(&[
        sample("claude-code", "UserPromptSubmit", json!({"prompt": "a", "workspace_roots": ["/a", "/b"]})),
        sample("claude-code", "UserPromptSubmit", json!({"prompt": "x".repeat(300)})),
        sample("claude-code", "UserPromptSubmit", json!({"prompt": ""})),
    ]);
    let prompt = field(&catalog, "claude-code", "UserPromptSubmit", "prompt");
    assert_eq!((prompt.count, prompt.sample.clone()), (3, Some(json!("x".repeat(SAMPLE_CHARS)))));
    assert_eq!(field(&catalog, "claude-code", "UserPromptSubmit", "workspace_roots[]").count, 2);
}

#[test]
fn a_documented_field_covers_what_is_nested_under_it() {
    let catalog = catalog_of(&[sample("claude-code", "PostToolUse", json!({"tool_input": {"file_path": "/a"}, "tool_input_size": 3}))]);
    assert!(field(&catalog, "claude-code", "PostToolUse", "tool_input.file_path").documented);
    assert!(!field(&catalog, "claude-code", "PostToolUse", "tool_input_size").documented);
}

#[test]
fn fields_documented_for_every_event_cover_each_event() {
    let catalog = catalog_of(&[sample("claude-code", "UserPromptSubmit", json!({"session_id": "s"}))]);
    assert!(field(&catalog, "claude-code", "UserPromptSubmit", "session_id").documented);
}

#[test]
fn documented_fields_not_seen_yet_are_listed_with_no_values() {
    let catalog = catalog_of(&[]);
    let reason = field(&catalog, "claude-code", "SessionEnd", "reason");
    assert_eq!((reason.count, reason.sample.clone(), reason.documented), (0, None, true));
    assert_eq!(field(&catalog, "codex", "*", "transcript_path").count, 0);
}

#[test]
fn a_seen_field_of_a_documented_name_is_not_listed_twice() {
    let catalog = catalog_of(&[sample("claude-code", "SessionEnd", json!({"reason": "clear"}))]);
    let found = catalog.fields.iter().filter(|f| f.source == "claude-code" && f.event == "SessionEnd" && f.path == "reason").count();
    assert_eq!((found, field(&catalog, "claude-code", "SessionEnd", "reason").count), (1, 1));
}

#[test]
fn a_source_without_a_spec_has_plain_seen_fields() {
    let catalog = catalog_of(&[sample("mystery", "x", json!({"a": 1}))]);
    let a = field(&catalog, "mystery", "x", "a");
    assert_eq!((a.count, a.sample.clone(), a.documented, a.unused.clone()), (1, Some(json!(1)), false, None));
}

#[test]
fn fields_are_sorted_by_source_event_and_path() {
    let catalog = catalog_of(&[sample("mystery", "b", json!({"z": 1, "a": 1})), sample("mystery", "a", json!({"y": 1}))]);
    let keys: Vec<(String, String, String)> = catalog.fields.iter().map(|f| (f.source.clone(), f.event.clone(), f.path.clone())).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn what_a_source_cannot_capture_or_drops_is_listed_with_the_reason() {
    let catalog = catalog_of(&[]);
    let codex = catalog.not_captured.iter().find(|n| n.source == "codex" && n.event == "PreToolUse").map(|n| n.reason.clone());
    let dropped =
        catalog.not_captured.iter().find(|n| n.source == "claude-code-otel" && n.event == "hook_registered").map(|n| n.reason.clone());
    assert!(codex.is_some_and(|r| r.contains("PostToolUse has the same data")));
    assert!(dropped.is_some_and(|r| r.starts_with("Not stored.")));
}

#[test]
fn the_fields_page_lists_every_derived_value() {
    assert_eq!(catalog_of(&[]).derived, derivations::docs());
}
