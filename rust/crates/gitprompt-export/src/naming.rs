//! The name of a saved export file: `gitprompt-<kind>[-<title>]-<day>.<ext>`, ASCII only, safe in a
//! `Content-Disposition` header. Port of `_filename` in `api/routes/export.py`.

use jiff::civil::Date;

use crate::formats::Format;
use crate::report::Report;

/// Characters of the title slug kept.
pub const SLUG_CHARS: usize = 40;

/// The title as a slug: lower case, every run of characters other than `a-z0-9` a single `-`,
/// trimmed of `-`, without a leading `<kind>-`, cut to [`SLUG_CHARS`].
pub fn slug(_report: &Report) -> String {
    todo!()
}

/// `gitprompt-<kind>-<slug>-<day><extension>`; the slug is left out when empty or equal to the
/// kind (`Overview`). `day` is the caller's local date (no clock here).
pub fn filename(_report: &Report, _format: Format, _day: Date) -> String {
    todo!()
}
