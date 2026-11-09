//! A report as plain data: what every document format renders (ADR-0018). Builders ([`crate::build`])
//! turn redacted input into a [`Report`] and its rows; formats ([`crate::markdown`], [`crate::html`],
//! [`crate::tabular`], [`crate::charts`]) turn a [`Report`] into a file. Neither side knows the other.
//!
//! Port of `export/report.py`.

use serde::{Deserialize, Serialize};

/// One headline number, e.g. `Prompts: 12`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Figure {
    pub label: String,
    pub value: String,
}

/// A row of headline numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Figures {
    pub items: Vec<Figure>,
}

/// One item of a chart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub label: String,
    pub value: f64,
    /// The value as read, e.g. `"$1.20"` or `"12"`.
    pub display: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    /// One horizontal bar per item, longest first.
    Bars,
    /// One column per item, in order (a time series); only a few labels are drawn.
    Columns,
}

impl ChartKind {
    /// `"bars"` or `"columns"` (the SVG's class is `gp-<name>`).
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    /// Stable within a report: a single chart is exported by it.
    pub id: String,
    pub title: String,
    pub kind: ChartKind,
    pub items: Vec<Bar>,
    pub caption: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub text: String,
    pub muted: bool,
}

/// Whose words a quote holds: what you typed, or system text (never shown as typed, ADR-0010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    You,
    System,
}

/// Text from the data, shown as quoted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub text: String,
    pub voice: Voice,
    /// For `You`: a note after "You typed" (e.g. `clipped`); for `System`: who wrote it, replacing
    /// "System text".
    pub caption: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Items {
    pub items: Vec<String>,
    /// File paths, commit hashes: set in a monospace font (Markdown: inline code).
    pub mono: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Per column: right-aligned. Shorter than `columns` = the rest are not numeric.
    pub numeric: Vec<bool>,
}

/// One block of a section, in reading order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Figures(Figures),
    Chart(Chart),
    Paragraph(Paragraph),
    Quote(Quote),
    Items(Items),
    Table(Table),
}

/// Heading level of a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Level {
    /// `h2` / `##`
    Two,
    /// `h3` / `###`
    Three,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub blocks: Vec<Block>,
    pub subtitle: String,
    pub level: Level,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub kind: crate::Kind,
    pub title: String,
    pub subtitle: String,
    /// Local time, as read (`build::when`).
    pub generated_at: String,
    pub sections: Vec<Section>,
    /// What this export includes and leaves out, printed in the file itself.
    pub notes: Vec<String>,
}

impl Report {
    /// Every chart of the report, in section then block order (including charts without items).
    pub fn charts(&self) -> Vec<&Chart> {
        todo!()
    }

    /// The chart with this id, if any.
    pub fn chart(&self, _id: &str) -> Option<&Chart> {
        todo!()
    }
}
