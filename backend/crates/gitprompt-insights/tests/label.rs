//! The default name of a session (label.rs): what you first typed, without the commands and paths
//! it starts with, short enough for a list row.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

use pretty_assertions::assert_eq;

use gitprompt_insights::label::{LABEL_CHARS, label, session_label};

#[test]
fn a_short_prompt_is_its_own_label() {
    assert_eq!(label("fix the parser").as_deref(), Some("fix the parser"));
}

#[test]
fn white_space_collapses_to_single_spaces() {
    assert_eq!(label("  fix \n\n the\tparser  ").as_deref(), Some("fix the parser"));
}

#[test]
fn a_leading_slash_command_is_dropped() {
    assert_eq!(label("/review the parser changes").as_deref(), Some("the parser changes"));
}

#[test]
fn a_leading_path_is_dropped() {
    for path in ["/Users/me/code/app/src/lib.rs", "~/notes/todo.md", "./src/main.rs", "../shared/util.ts", "src/lib.rs", "a/b/c"] {
        assert_eq!(label(&format!("{path} is failing to compile")).as_deref(), Some("is failing to compile"), "{path}");
    }
}

#[test]
fn a_leading_word_with_one_slash_and_no_file_name_is_a_word() {
    assert_eq!(label("and/or both work").as_deref(), Some("and/or both work"));
}

#[test]
fn a_command_then_a_path_are_both_dropped_but_a_later_path_stays() {
    assert_eq!(label("/fix /src/lib.rs handle ~/config.toml").as_deref(), Some("handle ~/config.toml"));
}

#[test]
fn a_prompt_with_nothing_but_a_command_or_nothing_at_all_has_no_label() {
    for text in ["", "   \n\t", "/clear", "/review /src/lib.rs"] {
        assert_eq!(label(text), None, "{text:?}");
    }
}

#[test]
fn a_long_prompt_is_cut_at_a_word_with_an_ellipsis_within_the_limit() {
    let text = "refactor the session list so that every card reads like a note you wrote yourself and not like a log line";
    let cut = label(text).unwrap();
    assert_eq!(cut, "refactor the session list so that every card reads like a…");
    assert!(cut.chars().count() <= LABEL_CHARS);
    assert!(text.starts_with(cut.trim_end_matches('…').trim_end()));
}

#[test]
fn text_of_exactly_the_limit_is_not_cut() {
    let text = "a".repeat(LABEL_CHARS - 5) + " word";
    assert_eq!(text.chars().count(), LABEL_CHARS);
    assert_eq!(label(&text).as_deref(), Some(text.as_str()));
    let longer = text + "s";
    assert!(label(&longer).unwrap().ends_with('…'));
}

#[test]
fn one_very_long_word_is_cut_inside_it() {
    let cut = label(&"x".repeat(200)).unwrap();
    assert_eq!(cut.chars().count(), LABEL_CHARS);
    assert!(cut.ends_with('…'));
}

#[test]
fn the_limit_counts_characters_not_bytes() {
    let cut = label(&"é".repeat(200)).unwrap();
    assert_eq!(cut.chars().count(), LABEL_CHARS);
}

#[test]
fn a_session_takes_the_title_its_tool_reported_before_anything_you_typed() {
    assert_eq!(session_label(Some("Fix the parser"), ["something else"]).as_deref(), Some("Fix the parser"));
}

#[test]
fn a_session_without_a_title_takes_the_first_typed_prompt_that_has_a_label() {
    assert_eq!(session_label(None, ["/clear", "", "now fix the parser", "later"]).as_deref(), Some("now fix the parser"));
    assert_eq!(session_label(Some("  "), ["fix it"]).as_deref(), Some("fix it"));
}

#[test]
fn a_session_with_nothing_to_go_on_has_no_label() {
    assert_eq!(session_label(None, ["/clear"]), None);
    assert_eq!(session_label(None, []), None);
}
