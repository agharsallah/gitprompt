//! The name of a saved export file: `gitprompt-<kind>[-<title>]-<day>.<ext>`, ASCII only, safe in a
//! `Content-Disposition` header. Port of `_filename` in `api/routes/export.py`.

use jiff::civil::Date;

use crate::formats::Format;
use crate::report::Report;

/// Characters of the title slug kept.
pub const SLUG_CHARS: usize = 40;

/// The title as a slug: lower case, every run of characters other than `a-z0-9` a single `-`,
/// trimmed of `-`, without a leading `<kind>-`, cut to [`SLUG_CHARS`].
pub fn slug(report: &Report) -> String {
    let mut dashed = String::with_capacity(report.title.len());
    for c in report.title.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            dashed.push(c);
        } else if !dashed.ends_with('-') {
            dashed.push('-');
        }
    }
    let trimmed = dashed.trim_matches('-');
    let prefix = format!("{}-", report.kind.as_str());
    // ASCII only by construction, so cutting at a byte index is cutting at a character.
    let slug = trimmed.strip_prefix(&prefix).unwrap_or(trimmed);
    slug[..slug.len().min(SLUG_CHARS)].to_string()
}

/// `gitprompt-<kind>-<slug>-<day><extension>`; the slug is left out when empty or equal to the
/// kind (`Overview`). `day` is the caller's local date (no clock here).
pub fn filename(report: &Report, format: Format, day: Date) -> String {
    let kind = report.kind.as_str();
    let slug = slug(report);
    let title = if slug.is_empty() || slug == kind { String::new() } else { format!("-{slug}") };
    format!("gitprompt-{kind}{title}-{}{}", day.strftime("%Y-%m-%d"), format.extension())
}
