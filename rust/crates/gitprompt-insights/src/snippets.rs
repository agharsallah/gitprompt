//! Search result snippets: the part of a typed text around the first match, with every match of
//! every searched word marked. Matching is the same as the `q` filter: each word as a substring,
//! ignoring case. A pure function, so the dashboard renders `<mark>` from data, never HTML. Positions
//! and widths count characters, not bytes.

use serde::{Deserialize, Serialize};

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
pub fn highlight(_text: Option<&str>, _q: Option<&str>, _width: usize) -> Vec<Segment> {
    todo!()
}
