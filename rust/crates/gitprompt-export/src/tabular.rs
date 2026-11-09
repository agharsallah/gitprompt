//! The rows under a report, as CSV or JSON: one row per chapter, prompt or statistics bucket.
//! Port of `export/tabular.py`.

use crate::Export;

/// A cell starting with one of these runs as a formula in a spreadsheet: such a string cell is
/// prefixed with an apostrophe (`'=SUM(A1)`). Numbers are never prefixed (`-3` stays a number).
pub const FORMULA: &[char] = &['=', '+', '-', '@', '\t', '\r'];

/// The CSV columns: every key of every row, in first-seen order.
pub fn columns(_rows: &[crate::Row]) -> Vec<String> {
    todo!()
}

/// CSV (RFC 4180 quoting, `\n` line ends): a header row, then one line per row. A missing or null
/// value is an empty cell; booleans `true`/`false`; an object or array is its compact JSON; a
/// field holding a comma, quote, CR or LF is quoted with `""` for quotes; formula cells are
/// neutralized ([`FORMULA`]), header cells included (an enhancement over the Python
/// daemon, which wrote the header as is). No rows: an empty header line.
pub fn csv(_export: &Export) -> String {
    todo!()
}

/// JSON, indented by two spaces, non-ASCII kept, ending with a newline: `kind`, `title`, `subtitle`,
/// `generated_at`, `options`, `notes`, `rows` in that order.
pub fn json(_export: &Export) -> String {
    todo!()
}
