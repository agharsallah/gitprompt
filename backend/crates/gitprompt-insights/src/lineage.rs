//! Prompt lineage: how similar two typed prompts are, how the wording changed, and whether the
//! outcome got better (ADR-0016). Pure functions: no embeddings, nothing leaves the machine.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use gitprompt_store::Outcome;

use crate::difflib::{Tag, opcodes};
use crate::text::{is_space, is_word, words};

/// Two typed texts are versions of one prompt from this similarity on (ADR-0016).
pub const THRESHOLD: f64 = 0.5;
/// Texts whose normalized lengths differ more than this ratio (shorter / longer below it) are not
/// versions of each other.
pub const LENGTH_RATIO: f64 = 0.5;
/// Texts are compared and diffed on their first characters only.
pub const COMPARED_CHARS: usize = 4000;

/// Lower case, punctuation (anything not a word character or whitespace) as spaces, whitespace
/// collapsed to single spaces and trimmed; only the first [`COMPARED_CHARS`] characters are read.
pub fn normalize(text: &str) -> String {
    let head: String = text.chars().take(COMPARED_CHARS).collect();
    let spaced: String = head.to_lowercase().chars().map(|c| if is_word(c) || is_space(c) { c } else { ' ' }).collect();
    words(&spaced).collect::<Vec<_>>().join(" ")
}

/// A text's normalized form, character trigrams and words, computed once per text (a story compares
/// one text against many).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shingles {
    pub normal: String,
    /// Every 3-character window of `normal` (a 1- or 2-character text is its own single shingle;
    /// none for an empty text).
    pub trigrams: BTreeSet<String>,
    pub words: BTreeSet<String>,
}

impl Shingles {
    pub fn new(text: &str) -> Self {
        let normal = normalize(text);
        let chars: Vec<char> = normal.chars().collect();
        let trigrams = match chars.len() {
            0 => BTreeSet::new(),
            1..=3 => BTreeSet::from([normal.clone()]),
            _ => chars.windows(3).map(|w| w.iter().collect()).collect(),
        };
        let words = words(&normal).map(String::from).collect();
        Shingles { normal, trigrams, words }
    }

    /// 0..=1, rounded to 3 decimals: the mean of the trigram and the word Jaccard indexes. 1 for equal
    /// normalized texts, 0 when either is empty or the lengths differ beyond [`LENGTH_RATIO`].
    /// Symmetric.
    pub fn similarity(&self, other: &Shingles) -> f64 {
        if self.normal.is_empty() || other.normal.is_empty() {
            return 0.0;
        }
        if self.normal == other.normal {
            return 1.0;
        }
        let (a, b) = (self.normal.chars().count() as f64, other.normal.chars().count() as f64);
        if a.min(b) / a.max(b) < LENGTH_RATIO {
            return 0.0;
        }
        crate::text::round((jaccard(&self.trigrams, &other.trigrams) + jaccard(&self.words, &other.words)) / 2.0, 3)
    }

    /// `similarity >= THRESHOLD`.
    pub fn is_version(&self, other: &Shingles) -> bool {
        self.similarity(other) >= THRESHOLD
    }
}

fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let shared = a.intersection(b).count();
    let union = a.len() + b.len() - shared;
    if union == 0 { 0.0 } else { shared as f64 / union as f64 }
}

/// [`Shingles::similarity`] of two texts.
pub fn similarity(a: &str, b: &str) -> f64 {
    Shingles::new(a).similarity(&Shingles::new(b))
}

/// [`Shingles::is_version`] of two texts.
pub fn is_version(a: &str, b: &str) -> bool {
    Shingles::new(a).is_version(&Shingles::new(b))
}

/// A run of a word diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Equal,
    Delete,
    Insert,
}

/// How `before` became `after`, token by token (whitespace runs, word runs and single punctuation
/// marks are tokens), as runs of equal, deleted and inserted text; adjacent runs of one op are
/// merged and a replaced run is a deletion followed by an insertion. The opcodes are those of
/// Python's `difflib.SequenceMatcher(None, a, b, autojunk=False)`, so the dashboard shows the same
/// diffs as before. Joining the non-insert runs gives `before` back, the non-delete runs `after`
/// (each cut to [`COMPARED_CHARS`]). `None` is an empty text. Serializes as `[op, text]` pairs.
pub fn word_diff(before: Option<&str>, after: Option<&str>) -> Vec<(Op, String)> {
    let (a, b) = (tokens(before.unwrap_or_default()), tokens(after.unwrap_or_default()));
    let mut out: Vec<(Op, String)> = Vec::new();
    let mut add = |op: Op, run: &[&str]| {
        if run.is_empty() {
            return;
        }
        match out.last_mut() {
            Some((last, text)) if *last == op => text.push_str(&run.concat()),
            _ => out.push((op, run.concat())),
        }
    };
    for (tag, i1, i2, j1, j2) in opcodes(&a, &b) {
        if tag == Tag::Equal {
            add(Op::Equal, &a[i1..i2]);
        } else {
            add(Op::Delete, &a[i1..i2]);
            add(Op::Insert, &b[j1..j2]);
        }
    }
    out
}

/// The diff tokens of a text's first [`COMPARED_CHARS`] characters: whitespace runs, word runs and
/// single other characters.
fn tokens(text: &str) -> Vec<&str> {
    let end = text.char_indices().nth(COMPARED_CHARS).map_or(text.len(), |(cut, _)| cut);
    let text = &text[..end];
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let class = |c: char| {
            if is_space(c) {
                0
            } else if is_word(c) {
                1
            } else {
                2
            }
        };
        let len = match class(first) {
            2 => first.len_utf8(),
            kind => rest.find(|c| class(c) != kind).unwrap_or(rest.len()),
        };
        out.push(&rest[..len]);
        rest = &rest[len..];
    }
    out
}

/// From one version's outcome to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Improved,
    Worse,
    Same,
}

/// Ranks outcomes: failed < interrupted = superseded = open < done. `running` has no verdict yet:
/// `None`, as when either outcome is unknown.
pub fn change(before: Option<Outcome>, after: Option<Outcome>) -> Option<Change> {
    let (before, after) = (rank(before?)?, rank(after?)?);
    Some(match after.cmp(&before) {
        std::cmp::Ordering::Greater => Change::Improved,
        std::cmp::Ordering::Less => Change::Worse,
        std::cmp::Ordering::Equal => Change::Same,
    })
}

/// How good an outcome is; `running` has no verdict yet.
fn rank(outcome: Outcome) -> Option<u8> {
    match outcome {
        Outcome::Failed => Some(0),
        Outcome::Interrupted | Outcome::Superseded | Outcome::Open => Some(1),
        Outcome::Done => Some(2),
        Outcome::Running => None,
    }
}
