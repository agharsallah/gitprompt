//! Personal data in text: email addresses, phone numbers, bank accounts, card numbers, and the
//! words you list yourself. Beside [`crate::secrets`]; together they are what "possible leaks" finds.
//!
//! Each kind is one [`Detector`] in [`DETECTORS`]: a cheap test, a pattern, and a check that turns a
//! look-alike away (a checksum, an example value, a line that only quotes commit metadata). Adding
//! a kind is adding a row and its table test. Precision comes first: a finding you learn to ignore
//! is worse than one not shown.

mod checks;
mod files;

use std::sync::LazyLock;

use fancy_regex::Regex;

pub use files::{SENSITIVE_FILES, sensitive_file};

use crate::leak::{Found, Mask, Scope};

/// One kind of personal data.
#[derive(Debug)]
pub struct Detector {
    pub name: &'static str,
    /// At least one must appear in the text before the pattern runs (none: always run).
    pub needles: &'static [&'static str],
    pub pattern: &'static str,
    /// Whether a match is the real thing, from the match and what surrounds it.
    pub check: fn(&Context<'_>) -> bool,
    pub mask: Mask,
    pub scope: Scope,
}

/// A match and what surrounds it. Patterns have no lookaround (which would run them on a slow
/// backtracking engine); the checks look at the characters around the match instead.
#[derive(Clone, Copy, Debug)]
pub struct Context<'t> {
    pub value: &'t str,
    /// The line the match is on.
    pub line: &'t str,
    /// About [`NEARBY`] characters on each side of the match, across lines.
    pub nearby: &'t str,
    /// The characters right before and after it.
    pub before: Option<char>,
    pub after: Option<char>,
}

/// Characters on each side of a match in [`Context::nearby`].
pub const NEARBY: usize = 160;

/// Every kind, in the order findings are reported.
pub const DETECTORS: &[Detector] = &[
    Detector {
        name: "email address",
        needles: &["@"],
        pattern: r"[A-Za-z0-9][A-Za-z0-9._%+_-]*@(?:[A-Za-z0-9-]+\.)+[A-Za-z]{2,24}\b",
        check: checks::email,
        mask: Mask::Email,
        scope: Scope::Anywhere,
    },
    Detector {
        name: "phone number",
        needles: &["+"],
        pattern: r"\+\d[\d .()-]{6,20}\d",
        check: checks::phone,
        mask: Mask::Phone,
        scope: Scope::Sent,
    },
    Detector {
        name: "bank account (IBAN)",
        needles: &[],
        pattern: r"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?\b",
        check: checks::iban,
        mask: Mask::Account,
        scope: Scope::Anywhere,
    },
    Detector {
        name: "card number",
        needles: &[],
        pattern: r"(?:\d[ -]?){12,18}\d",
        check: checks::card,
        mask: Mask::Card,
        scope: Scope::Anywhere,
    },
];

/// The name of a word from your own list.
pub const PRIVATE_TERM: &str = "private term";

static COMPILED: LazyLock<Vec<Regex>> = LazyLock::new(|| DETECTORS.iter().map(compile).collect());

#[expect(clippy::expect_used, reason = "DETECTORS are constants, each compiled by `every_detector_compiles`")]
fn compile(detector: &Detector) -> Regex {
    Regex::new(detector.pattern).expect("personal data patterns are valid")
}

/// Every personal value in `text`, kind by kind; a value that fails its check is left out.
pub fn find_personal(text: &str) -> Vec<Found<'_>> {
    DETECTORS.iter().zip(COMPILED.iter()).flat_map(|(detector, regex)| matches(detector, regex, text)).collect()
}

fn matches<'t>(detector: &Detector, regex: &Regex, text: &'t str) -> Vec<Found<'t>> {
    if !detector.needles.is_empty() && !detector.needles.iter().any(|needle| text.contains(needle)) {
        return Vec::new();
    }
    regex
        .find_iter(text)
        .filter_map(Result::ok)
        .filter(|m| {
            let before = text.get(..m.start()).and_then(|s| s.chars().next_back());
            let after = text.get(m.end()..).and_then(|s| s.chars().next());
            let (line, nearby) = (line_around(text, m.start(), m.end()), nearby(text, m.start(), m.end()));
            (detector.check)(&Context { value: m.as_str(), line, nearby, before, after })
        })
        .map(|m| Found { kind: detector.name, value: m.as_str(), mask: detector.mask, scope: detector.scope })
        .collect()
}

/// The line holding `start..end`.
fn line_around(text: &str, start: usize, end: usize) -> &str {
    let from = text.get(..start).and_then(|before| before.rfind('\n')).map_or(0, |at| at + 1);
    let to = text.get(end..).and_then(|after| after.find('\n')).map_or(text.len(), |at| end + at);
    text.get(from..to).unwrap_or_default()
}

/// About [`NEARBY`] characters each side of `start..end` (on character boundaries).
fn nearby(text: &str, start: usize, end: usize) -> &str {
    let mut from = start.saturating_sub(NEARBY);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (end + NEARBY).min(text.len());
    while !text.is_char_boundary(to) {
        to += 1;
    }
    text.get(from..to).unwrap_or_default()
}

/// Every whole-word occurrence of a word from your list (`terms`, any case; blank ones ignored).
pub fn find_terms<'t>(text: &'t str, terms: &[String]) -> Vec<Found<'t>> {
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths (rare letters); then match on the text itself.
    let haystack = if lower.len() == text.len() { lower.as_str() } else { text };
    let mut out = Vec::new();
    for term in terms.iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()) {
        let mut from = 0;
        while let Some(at) = haystack.get(from..).and_then(|rest| rest.find(&term)).map(|at| from + at) {
            let end = at + term.len();
            if is_word_edge(text, at, end)
                && let Some(value) = text.get(at..end)
            {
                out.push(Found { kind: PRIVATE_TERM, value, mask: Mask::Token, scope: Scope::Anywhere });
            }
            from = end;
        }
    }
    out
}

/// No word character right before `start` or right after `end`.
fn is_word_edge(text: &str, start: usize, end: usize) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let before = text.get(..start).and_then(|s| s.chars().next_back());
    let after = text.get(end..).and_then(|s| s.chars().next());
    !before.is_some_and(word) && !after.is_some_and(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_detector_compiles() {
        for detector in DETECTORS {
            assert!(Regex::new(detector.pattern).is_ok(), "{}", detector.name);
        }
    }

    #[test]
    fn a_match_is_checked_against_its_own_line() {
        assert_eq!(line_around("a\nb x c\nd", 4, 5), "b x c");
        assert_eq!(line_around("only", 0, 4), "only");
    }
}
