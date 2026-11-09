//! A report as one self-contained HTML file (ADR-0018 §5): inline CSS, inline SVG charts, no
//! script, font, image or link. A Content-Security-Policy in the file itself forbids every request,
//! so even a value that slipped through can't make the page load anything. Follows the reader's
//! light or dark setting (`prefers-color-scheme`) and prints on white (`@media print` forces the
//! light palette and hides the "Numbers" disclosures). Port of `export/html.py`.

use crate::report::Report;

/// The page's own policy; the API sends the same value as a header.
pub const CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'";

/// The page's stylesheet: both palettes as CSS variables (`--bg`, `--text`... and the charts'
/// `--gp-*`), `color-scheme: light dark`, the dark palette under `prefers-color-scheme: dark`, the
/// light one again under `@media print`.
pub fn style() -> String {
    todo!()
}

/// The report as one HTML document (`<!doctype html>` ... `</html>\n`). Every text is escaped;
/// consecutive charts share one `div.charts` grid, each a `figure` with its inline SVG and a
/// "Numbers" table; a chart or table without rows is left out.
pub fn html(_report: &Report) -> String {
    todo!()
}
