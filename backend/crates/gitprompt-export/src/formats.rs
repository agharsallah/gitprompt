//! The formats an export renders to (ADR-0018 §4). A closed enum rather than Python's runtime
//! registry: adding a format is a compile error everywhere it must be handled ([`crate::render`]),
//! which is the Rust form of "a new format touches no other module by accident".
//!
//! Port of `export/formats.py` (+ the `@register` metadata of each format module).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// GitHub-flavoured Markdown; charts become tables.
    #[serde(rename = "md")]
    Markdown,
    /// One self-contained file: inline CSS and SVG, no script, its own CSP.
    Html,
    /// The rows, one per chapter, prompt or statistics bucket.
    Csv,
    /// The rows with the report's head and the options.
    Json,
    /// One chart, as a standalone SVG file.
    Svg,
    /// One chart, rasterized at 2× with resvg, on the light palette.
    Png,
}

impl Format {
    /// Every format, in the order the dashboard lists them.
    pub const ALL: [Format; 6] = [Format::Markdown, Format::Html, Format::Csv, Format::Json, Format::Svg, Format::Png];

    /// The `format` query value: `md`, `html`, `csv`, `json`, `svg`, `png`.
    pub fn name(self) -> &'static str {
        self.meta().name
    }

    /// The format of a `format` query value; `None` for an unknown one.
    pub fn parse(name: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|format| format.name() == name)
    }

    /// As listed in the dashboard: `Markdown`, `HTML`, `CSV`, `JSON`, `Chart (SVG)`, `Chart (PNG)`.
    pub fn label(self) -> &'static str {
        self.meta().label
    }

    /// The response's media type, e.g. `text/markdown; charset=utf-8`, `image/svg+xml`, `image/png`.
    pub fn media_type(self) -> &'static str {
        self.meta().media_type
    }

    /// The file extension, with its dot: `.md`, `.html`...
    pub fn extension(self) -> &'static str {
        self.meta().extension
    }

    /// Renders one chart of the report (`svg`, `png`) rather than the whole report.
    pub fn single_chart(self) -> bool {
        matches!(self, Format::Svg | Format::Png)
    }

    /// The body is UTF-8 text (every format but `png`).
    pub fn is_text(self) -> bool {
        self != Format::Png
    }

    /// The format's row of the table above, kept in one place so its names can't drift apart.
    fn meta(self) -> Meta {
        let (name, label, media_type, extension) = match self {
            Format::Markdown => ("md", "Markdown", "text/markdown; charset=utf-8", ".md"),
            Format::Html => ("html", "HTML", "text/html; charset=utf-8", ".html"),
            Format::Csv => ("csv", "CSV", "text/csv; charset=utf-8", ".csv"),
            Format::Json => ("json", "JSON", "application/json", ".json"),
            Format::Svg => ("svg", "Chart (SVG)", "image/svg+xml", ".svg"),
            Format::Png => ("png", "Chart (PNG)", "image/png", ".png"),
        };
        Meta { name, label, media_type, extension }
    }
}

/// What a format is called and served as.
struct Meta {
    name: &'static str,
    label: &'static str,
    media_type: &'static str,
    extension: &'static str,
}
