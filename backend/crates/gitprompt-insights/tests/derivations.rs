//! Derived values of a prompt (derivations.rs): its typed text's stats, the files a tool touched,
//! and `derive`. Port of `tests/insights/test_insights.py` (non-secret parts) plus every branch of
//! derivations.py it left implicit. The turn and session summaries: `turn_summary.rs`,
//! `session_summary.rs`.
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

use common::{event, prompt, spec, strings};
use gitprompt_core::spec::Pattern;
use gitprompt_insights::derivations::{PromptStats, derive, files_of, prompt_stats};

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
    let docs = gitprompt_insights::catalog::derived_docs();
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
