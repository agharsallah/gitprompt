//! A report as Markdown (GitHub flavoured): charts become tables of their values; text placed in a
//! sentence has its Markdown escaped, and quoted prompt text goes in a fence long enough to hold
//! it, so its links and images stay inert. Port of `export/markdown.py`.

use crate::report::Report;

/// The report as one Markdown document: `# title`, subtitle, sections (`##`/`###`), then a `---`
/// footer with the export date and notes. Ends with a newline.
pub fn markdown(_report: &Report) -> String {
    todo!()
}

/// Text placed in a sentence: `\ ` `` ` `` `* _ [ ] < > ! #` are backslash-escaped, so nothing can
/// turn into a link, an image or a heading.
pub fn inline(_text: &str) -> String {
    todo!()
}

/// A table cell: whitespace runs (newlines included) become one space, `|` is escaped, trimmed;
/// an empty cell is one space.
pub fn cell(_text: &str) -> String {
    todo!()
}

/// `text` in a fence of backticks one longer than its longest backtick run (at least three),
/// tagged `text`.
pub fn fence(_text: &str) -> String {
    todo!()
}
