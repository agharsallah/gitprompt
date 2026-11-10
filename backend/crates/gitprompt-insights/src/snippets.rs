//! Search result snippets: the part of a typed text around the first match, with every match of
//! every searched word marked. Matching is the same as the `q` filter: each word as a substring,
//! ignoring case. A pure function, so the dashboard renders `<mark>` from data, never HTML. Positions
//! and widths count characters, not bytes.

use serde::{Deserialize, Serialize};

use crate::text::{is_space, words};

/// Characters of context shown.
pub const WIDTH: usize = 180;
pub const ELLIPSIS: &str = "…";

/// A piece of a snippet; `hit` pieces are matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub hit: bool,
}

/// Segments of a window of `text` of about `width` characters:
///
/// * matches of the query's words (split on whitespace, case-insensitive, duplicates ignored);
///   overlapping or touching matches are merged into one hit
/// * the window starts at 0, or, when the first match begins after `width / 3`, `width / 3`
///   characters before it, moved forward to just after the next whitespace before the match
/// * it ends `width` characters later, extended to the end of the first match if that is cut, else
///   moved back to a whitespace within the last 30 characters
/// * whitespace runs inside segments are flattened to one space
/// * [`ELLIPSIS`] segments mark text cut before and after the window
///
/// Without a match (or a query) the start of the text; `[]` for no or empty text.
pub fn highlight(text: Option<&str>, q: Option<&str>, width: usize) -> Vec<Segment> {
    let text: Vec<char> = text.unwrap_or_default().chars().collect();
    let mut query: Vec<Vec<char>> = Vec::new();
    for word in words(q.unwrap_or_default()).map(|w| w.to_lowercase().chars().collect::<Vec<char>>()) {
        if !query.contains(&word) {
            query.push(word);
        }
    }
    let spans = spans(&text, &query);
    let (start, end) = window(&text, &spans, width);
    let piece = |from: usize, to: usize, hit: bool| Segment { text: flat(&text[from..to]), hit };
    let mut out = Vec::new();
    let mut position = start;
    for &(s, e) in spans.iter().filter(|(s, e)| *e > start && *s < end) {
        let (s, e) = (s.max(start), e.min(end));
        if s > position {
            out.push(piece(position, s, false));
        }
        out.push(piece(s, e, true));
        position = e;
    }
    if position < end {
        out.push(piece(position, end, false));
    }
    if !out.is_empty() && start > 0 {
        out.insert(0, Segment { text: ELLIPSIS.into(), hit: false });
    }
    if !out.is_empty() && end < text.len() {
        out.push(Segment { text: ELLIPSIS.into(), hit: false });
    }
    out
}

/// `(start, end)` character positions of the words' matches, ignoring case, overlapping and
/// touching matches merged, in order.
fn spans(text: &[char], words: &[Vec<char>]) -> Vec<(usize, usize)> {
    let mut found: Vec<(usize, usize)> = Vec::new();
    for word in words.iter().filter(|w| !w.is_empty()) {
        let mut at = 0;
        while at + word.len() <= text.len() {
            if text[at..at + word.len()].iter().zip(word).all(|(t, w)| same_letter(*t, *w)) {
                found.push((at, at + word.len()));
                at += word.len();
            } else {
                at += 1;
            }
        }
    }
    found.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in found {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Equal ignoring case (simple case folding: both lower-cased).
fn same_letter(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// The `(start, end)` of the window shown, in characters.
fn window(text: &[char], spans: &[(usize, usize)], width: usize) -> (usize, usize) {
    let lead = width / 3;
    let mut start = 0;
    if let Some(&(first, _)) = spans.first().filter(|(first, _)| *first > lead) {
        start = first - lead;
        // Start on a word: just after the next whitespace before the match.
        start = (start..first).find(|&i| is_space(text[i])).map_or(start, |i| i + 1);
    }
    let mut end = text.len().min(start + width);
    if let Some(&(_, first_end)) = spans.first().filter(|(_, e)| end < text.len() && *e > end) {
        end = first_end; // never cut the first match
    }
    if end < text.len() {
        // End on whitespace within the last 30 characters.
        end = (start.max(end.saturating_sub(30)) + 1..=end).rev().find(|&i| is_space(text[i])).unwrap_or(end);
    }
    (start, end)
}

/// Whitespace runs flattened to one space.
fn flat(text: &[char]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for &c in text {
        if is_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}
