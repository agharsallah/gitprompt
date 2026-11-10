//! The rows under a report, as CSV or JSON: one row per chapter, prompt or statistics bucket.
//! Port of `export/tabular.py`.

use gitprompt_core::Json;

use crate::{Export, Row};

/// A cell starting with one of these runs as a formula in a spreadsheet: such a string cell is
/// prefixed with an apostrophe (`'=SUM(A1)`). Numbers are never prefixed (`-3` stays a number).
pub const FORMULA: &[char] = &['=', '+', '-', '@', '\t', '\r'];

/// The CSV columns: every key of every row, in first-seen order.
pub fn columns(rows: &[Row]) -> Vec<String> {
    let mut seen = indexmap::IndexSet::new();
    for row in rows {
        seen.extend(row.keys().cloned());
    }
    seen.into_iter().collect()
}

/// CSV (RFC 4180 quoting, `\n` line ends): a header row, then one line per row. A missing or null
/// value is an empty cell; booleans `true`/`false`; an object or array is its compact JSON; a
/// field holding a comma, quote, CR or LF is quoted with `""` for quotes; formula cells are
/// neutralized ([`FORMULA`]), header cells included (an enhancement over the Python
/// daemon, which wrote the header as is). No rows: an empty header line.
pub fn csv(export: &Export) -> String {
    let columns = columns(&export.rows);
    let mut out = line(columns.iter().map(|c| text_cell(c)));
    for row in &export.rows {
        out.push_str(&line(columns.iter().map(|c| value_cell(row.get(c)))));
    }
    out
}

/// One CSV line of already written cells.
fn line(cells: impl Iterator<Item = String>) -> String {
    let mut line = cells.collect::<Vec<_>>().join(",");
    line.push('\n');
    line
}

/// A JSON value as a CSV field.
fn value_cell(value: Option<&Json>) -> String {
    match value {
        None | Some(Json::Null) => String::new(),
        Some(Json::String(text)) => text_cell(text),
        Some(Json::Bool(b)) => b.to_string(),
        Some(Json::Number(n)) => n.to_string(),
        Some(nested) => text_cell(&nested.to_string()),
    }
}

/// The one CSV text cell writer: a formula neutralized, then quoted when it must be.
fn text_cell(text: &str) -> String {
    let text = if text.starts_with(FORMULA) { format!("'{text}") } else { text.to_string() };
    if text.contains([',', '"', '\r', '\n']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
}

/// JSON, indented by two spaces, non-ASCII kept, ending with a newline: `kind`, `title`, `subtitle`,
/// `generated_at`, `options`, `notes`, `rows` in that order.
pub fn json(export: &Export) -> String {
    let report = &export.report;
    let document = serde_json::json!({
        "kind": report.kind.as_str(),
        "title": report.title,
        "subtitle": report.subtitle,
        "generated_at": report.generated_at,
        "options": export.options.as_json(),
        "notes": report.notes,
        "rows": export.rows,
    });
    // A `Value` always serializes (string keys, finite numbers), so this cannot fail.
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_default();
    text.push('\n');
    text
}
