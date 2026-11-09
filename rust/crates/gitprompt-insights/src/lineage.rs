//! Prompt lineage: how similar two typed prompts are, how the wording changed, and whether the
//! outcome got better (ADR-0016). Pure functions: no embeddings, nothing leaves the machine.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use gitprompt_store::Outcome;

/// Two typed texts are versions of one prompt from this similarity on (ADR-0016).
pub const THRESHOLD: f64 = 0.5;
/// Texts whose normalized lengths differ more than this ratio (shorter / longer below it) are not
/// versions of each other.
pub const LENGTH_RATIO: f64 = 0.5;
/// Texts are compared and diffed on their first characters only.
pub const COMPARED_CHARS: usize = 4000;

/// Lower case, punctuation (anything not a word character or whitespace) as spaces, whitespace
/// collapsed to single spaces and trimmed; only the first [`COMPARED_CHARS`] characters are read.
pub fn normalize(_text: &str) -> String {
    todo!()
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
    pub fn new(_text: &str) -> Self {
        todo!()
    }

    /// 0..=1, rounded to 3 decimals: the mean of the trigram and the word Jaccard indexes. 1 for equal
    /// normalized texts, 0 when either is empty or the lengths differ beyond [`LENGTH_RATIO`].
    /// Symmetric.
    pub fn similarity(&self, _other: &Shingles) -> f64 {
        todo!()
    }

    /// `similarity >= THRESHOLD`.
    pub fn is_version(&self, _other: &Shingles) -> bool {
        todo!()
    }
}

/// [`Shingles::similarity`] of two texts.
pub fn similarity(_a: &str, _b: &str) -> f64 {
    todo!()
}

/// [`Shingles::is_version`] of two texts.
pub fn is_version(_a: &str, _b: &str) -> bool {
    todo!()
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
pub fn word_diff(_before: Option<&str>, _after: Option<&str>) -> Vec<(Op, String)> {
    todo!()
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
pub fn change(_before: Option<Outcome>, _after: Option<Outcome>) -> Option<Change> {
    todo!()
}
