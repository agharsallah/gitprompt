//! Search snippets (snippets.rs): a window around the first match, every match marked. Port of
//! tests/insights/test_snippets.py plus the window boundaries.

use pretty_assertions::assert_eq;

use gitprompt_insights::snippets::{ELLIPSIS, Segment, WIDTH, highlight};

fn text_of(segments: &[Segment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect()
}

fn hits(segments: &[Segment]) -> Vec<&str> {
    segments.iter().filter(|s| s.hit).map(|s| s.text.as_str()).collect()
}

fn seg(text: &str, hit: bool) -> Segment {
    Segment { text: text.into(), hit }
}

#[test]
fn every_word_is_marked_ignoring_case() {
    let out = highlight(Some("Fix the Parser, then test the parser again"), Some("parser test"), WIDTH);
    assert_eq!(hits(&out), vec!["Parser", "test", "parser"]);
    assert_eq!(text_of(&out), "Fix the Parser, then test the parser again");
}

#[test]
fn overlapping_matches_are_one_mark() {
    assert_eq!(hits(&highlight(Some("a configuration file"), Some("config configuration"), WIDTH)), vec!["configuration"]);
}

#[test]
fn touching_matches_are_one_mark() {
    assert_eq!(hits(&highlight(Some("foobar"), Some("foo bar"), WIDTH)), vec!["foobar"]);
}

#[test]
fn the_window_starts_near_the_first_match_on_a_word_boundary() {
    let text = format!("{}the needle is here {}", "lorem ipsum ".repeat(40), "dolor sit ".repeat(40));
    let out = highlight(Some(&text), Some("needle"), 80);
    assert_eq!((out.first(), out.last()), (Some(&seg(ELLIPSIS, false)), Some(&seg(ELLIPSIS, false))));
    assert_eq!(hits(&out), vec!["needle"]);
    assert!(!out[1].text.starts_with("orem") && !out[1].text.starts_with("psum"), "{:?}", out[1]);
    assert!(text_of(&out).chars().count() <= 80 + 2);
}

#[test]
fn a_match_early_in_the_text_keeps_the_start() {
    let out = highlight(Some(&format!("one two needle {}", "x ".repeat(200))), Some("needle"), 60);
    assert_eq!((out[0].clone(), out.last().cloned()), (seg("one two ", false), Some(seg(ELLIPSIS, false))));
}

#[test]
fn the_first_match_is_never_cut() {
    let out = highlight(Some("aaaaa supercalifragilistic tail words here"), Some("supercalifragilistic"), 10);
    assert_eq!(out, vec![seg(ELLIPSIS, false), seg("supercalifragilistic", true), seg(ELLIPSIS, false)]);
}

#[test]
fn the_window_ends_on_whitespace() {
    let out = highlight(Some(&"word ".repeat(60)), None, 23);
    assert_eq!(out, vec![seg("word word word word", false), seg(ELLIPSIS, false)]);
}

#[test]
fn spacing_is_flattened() {
    assert_eq!(text_of(&highlight(Some("one\n\n  two"), Some("two"), WIDTH)), "one two");
}

#[test]
fn without_a_match_or_a_query_the_start_is_shown() {
    assert_eq!(highlight(Some("short text"), Some("absent"), WIDTH), vec![seg("short text", false)]);
    assert_eq!(highlight(Some("short text"), None, WIDTH), vec![seg("short text", false)]);
    assert_eq!(highlight(Some("short text"), Some("   "), WIDTH), vec![seg("short text", false)]);
}

#[test]
fn no_text_gives_no_segments() {
    assert_eq!((highlight(None, Some("x"), WIDTH), highlight(Some(""), Some("x"), WIDTH)), (vec![], vec![]));
}

#[test]
fn short_words_match_like_the_filter() {
    assert_eq!(hits(&highlight(Some("go to the CI page"), Some("ci go"), WIDTH)), vec!["go", "CI"]);
}

#[test]
fn a_repeated_query_word_marks_once() {
    assert_eq!(highlight(Some("fix it"), Some("fix FIX"), WIDTH), vec![seg("fix", true), seg(" it", false)]);
}

#[test]
fn query_words_are_literal_text() {
    assert_eq!(hits(&highlight(Some("call f(x) or f.x"), Some("f(x) .x"), WIDTH)), vec!["f(x)", ".x"]);
}

#[test]
fn positions_count_characters_not_bytes() {
    assert_eq!(highlight(Some("ünïcode wörd ok"), Some("wörd"), WIDTH), vec![seg("ünïcode ", false), seg("wörd", true), seg(" ok", false)]);
}
