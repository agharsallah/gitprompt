//! Prompt lineage (lineage.rs): similarity, word diff and outcome change, on synthetic texts. Port of
//! tests/insights/test_lineage.py plus the length-ratio and shingle boundaries.

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use serde_json::json;

use gitprompt_insights::lineage::{COMPARED_CHARS, Change, Op, Shingles, change, is_version, normalize, similarity, word_diff};
use gitprompt_store::Outcome;

fn ops(pairs: &[(Op, &str)]) -> Vec<(Op, String)> {
    pairs.iter().map(|(op, t)| (*op, t.to_string())).collect()
}

#[test]
fn normalizing_ignores_case_punctuation_and_spacing() {
    assert_eq!(normalize("  Fix the  Login-bug, please!\n"), "fix the login bug please");
    assert_eq!(normalize(""), "");
}

#[test]
fn normalizing_reads_only_the_compared_characters() {
    assert!(normalize(&"x ".repeat(COMPARED_CHARS)).chars().count() <= COMPARED_CHARS);
}

#[test]
fn the_same_text_with_other_case_and_punctuation_is_a_version() {
    assert!(is_version("Fix the login bug", "fix the login bug!"));
}

#[test]
fn an_extended_instruction_is_a_version() {
    assert!(
        is_version("add tests for the parser", "add unit tests for the parser module"),
        "{}",
        similarity("add tests for the parser", "add unit tests for the parser module")
    );
    assert!(is_version("rename the config loader", "rename the config loader and its tests"));
}

#[test]
fn a_reworded_instruction_is_a_version() {
    assert!(
        is_version("explain the cache layer", "explain the caching layer"),
        "{}",
        similarity("explain the cache layer", "explain the caching layer")
    );
}

#[test]
fn different_asks_stay_below_the_threshold() {
    assert!(!is_version("open the settings page", "deploy the release"));
    assert!(!is_version("commit the parser", "deploy the cache"), "{}", similarity("commit the parser", "deploy the cache"));
}

#[test]
fn texts_of_very_different_lengths_are_never_versions() {
    let long = format!("summarize {}", "a long pasted brief with many words ".repeat(20));
    assert_eq!(similarity("summarize", &long), 0.0);
}

#[test]
fn an_empty_text_is_similar_to_nothing() {
    assert_eq!((similarity("", "anything"), similarity("", "")), (0.0, 0.0));
}

#[test]
fn a_length_ratio_of_exactly_one_half_is_still_compared() {
    assert_eq!(similarity("aaaa", "aaaa bbb"), 0.35);
    assert_eq!(similarity("aaaa", "aaaa bbbb"), 0.0);
}

#[test]
fn similarity_is_symmetric_and_bounded() {
    let (a, b) = ("write the changelog for 2.0", "write a changelog for version 2.0");
    assert_eq!(similarity(a, b), similarity(b, a));
    assert!(0.0 < similarity(a, b) && similarity(a, b) < 1.0);
    assert_eq!(similarity(a, &a.to_uppercase()), 1.0);
}

#[test]
fn similarity_is_rounded_to_three_decimals() {
    let s = similarity("write the changelog for 2.0", "write a changelog for version 2.0");
    assert_eq!((s * 1000.0).round() / 1000.0, s);
}

#[test]
fn long_texts_are_compared_on_their_start() {
    let head = "refactor the importer into small modules ".repeat(120);
    assert_eq!(similarity(&format!("{head}tail one"), &format!("{head}{}", "a completely different ending ".repeat(200))), 1.0);
}

#[test]
fn shingles_hold_trigrams_and_words_and_a_short_text_is_its_own_shingle() {
    let fix = Shingles::new("Fix it");
    assert_eq!(fix.normal, "fix it");
    assert_eq!(fix.trigrams.iter().map(String::as_str).collect::<Vec<_>>(), vec![" it", "fix", "ix ", "x i"]);
    assert_eq!(fix.words.iter().map(String::as_str).collect::<Vec<_>>(), vec!["fix", "it"]);
    assert_eq!(Shingles::new("ab").trigrams.iter().map(String::as_str).collect::<Vec<_>>(), vec!["ab"]);
    assert!(Shingles::new("!!").trigrams.is_empty());
}

#[test]
fn shingles_compare_like_their_texts() {
    let (a, b) = ("add tests for the parser", "add unit tests for the parser module");
    assert_eq!(Shingles::new(a).similarity(&Shingles::new(b)), similarity(a, b));
    assert!(Shingles::new(a).is_version(&Shingles::new(b)));
}

#[test]
fn a_word_diff_marks_what_was_removed_and_added() {
    assert_eq!(
        word_diff(Some("add tests for the parser"), Some("add unit tests for the lexer")),
        ops(&[(Op::Equal, "add"), (Op::Insert, " unit"), (Op::Equal, " tests for the "), (Op::Delete, "parser"), (Op::Insert, "lexer")])
    );
}

#[test]
fn an_unchanged_text_is_one_equal_run() {
    assert_eq!(word_diff(Some("same text"), Some("same text")), ops(&[(Op::Equal, "same text")]));
}

#[test]
fn a_missing_text_diffs_as_empty() {
    assert_eq!(word_diff(None, Some("new")), ops(&[(Op::Insert, "new")]));
    assert_eq!(word_diff(Some("old"), None), ops(&[(Op::Delete, "old")]));
    assert_eq!(word_diff(None, None), vec![]);
}

#[test]
fn a_word_diff_rebuilds_both_texts() {
    let (before, after) = ("Fix the bug, then\nrun the tests.", "Fix both bugs; then run all tests!");
    let diff = word_diff(Some(before), Some(after));
    let old: String = diff.iter().filter(|(op, _)| *op != Op::Insert).map(|(_, t)| t.as_str()).collect();
    let new: String = diff.iter().filter(|(op, _)| *op != Op::Delete).map(|(_, t)| t.as_str()).collect();
    assert_eq!((old.as_str(), new.as_str()), (before, after));
}

#[test]
fn a_word_diff_serializes_as_op_text_pairs() {
    assert_eq!(serde_json::to_value(word_diff(Some("same"), Some("same"))).unwrap(), json!([["equal", "same"]]));
}

proptest! {
    #[test]
    fn any_word_diff_rebuilds_both_texts(before in "[a-z ,.!\n]{0,40}", after in "[a-z ,.!\n]{0,40}") {
        let diff = word_diff(Some(&before), Some(&after));
        let old: String = diff.iter().filter(|(op, _)| *op != Op::Insert).map(|(_, t)| t.as_str()).collect();
        let new: String = diff.iter().filter(|(op, _)| *op != Op::Delete).map(|(_, t)| t.as_str()).collect();
        prop_assert_eq!((old, new), (before, after));
    }

    #[test]
    fn similarity_is_always_symmetric_and_between_zero_and_one(a in "[a-zA-Z ]{0,30}", b in "[a-zA-Z ]{0,30}") {
        let s = similarity(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s));
        prop_assert_eq!(s, similarity(&b, &a));
    }
}

#[test]
fn a_better_outcome_is_an_improvement() {
    assert_eq!(change(Some(Outcome::Failed), Some(Outcome::Done)), Some(Change::Improved));
    assert_eq!(change(Some(Outcome::Superseded), Some(Outcome::Done)), Some(Change::Improved));
    assert_eq!(change(Some(Outcome::Failed), Some(Outcome::Interrupted)), Some(Change::Improved));
}

#[test]
fn a_worse_outcome_is_worse() {
    assert_eq!(change(Some(Outcome::Done), Some(Outcome::Failed)), Some(Change::Worse));
    assert_eq!(change(Some(Outcome::Open), Some(Outcome::Failed)), Some(Change::Worse));
}

#[test]
fn unfinished_outcomes_rank_the_same() {
    assert_eq!(change(Some(Outcome::Open), Some(Outcome::Superseded)), Some(Change::Same));
    assert_eq!(change(Some(Outcome::Interrupted), Some(Outcome::Open)), Some(Change::Same));
}

#[test]
fn a_running_or_unknown_outcome_has_no_verdict() {
    assert_eq!(change(Some(Outcome::Done), Some(Outcome::Running)), None);
    assert_eq!(change(Some(Outcome::Running), Some(Outcome::Done)), None);
    assert_eq!(change(None, Some(Outcome::Done)), None);
}
