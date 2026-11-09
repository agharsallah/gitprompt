//! Exports (plan E5, ADR-0018): a session, a project, the Overview or a prompt selection as a file
//! you send yourself. Nothing leaves the machine; nothing here does I/O except [`collect`].
//!
//! ```text
//! ExportInput ──redact::redact──► clean input ──build::build──► Report + rows ──render(format)──► file
//! ```
//!
//! One redaction pass for every format ([`prepare`]); builders are pure; a format only sees the
//! [`Export`]. Port of `src/gitprompt/export/*` and the pure parts of `api/routes/export.py`.

use std::collections::BTreeSet;

use jiff::tz::Offset;
use serde::{Deserialize, Serialize};

use gitprompt_core::JsonMap;
use gitprompt_store::{PromptFilter, Store};

pub mod build;
pub mod charts;
pub mod formats;
pub mod html;
pub mod input;
pub mod markdown;
pub mod naming;
pub mod redact;
pub mod report;
pub mod tabular;

pub use charts::ChartInfo;
pub use formats::Format;
pub use input::ExportInput;
pub use redact::Options;
pub use report::Report;

/// A prompt selection exports at most this many prompts, the newest (the total is stated).
pub const MAX_PROMPTS: usize = 2000;
/// Buckets per grouping of an Overview export (days of a year, projects...).
pub const GROUP_LIMIT: usize = 400;

/// What is exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Session,
    Project,
    Overview,
    Prompts,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Session, Kind::Project, Kind::Overview, Kind::Prompts];

    /// `session`, `project`, `overview`, `prompts`.
    pub fn as_str(self) -> &'static str {
        todo!()
    }

    /// The kind of a path segment; `None` for an unknown one (the API answers 422).
    pub fn parse(_name: &str) -> Option<Kind> {
        todo!()
    }
}

/// The time zone instants are shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Zone {
    /// This computer's zone, DST included (production).
    #[default]
    Local,
    /// A fixed offset (tests, or a reader elsewhere).
    Fixed(Offset),
}

/// Where an export is made: what the redaction pass and the builders need besides the data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// The user's home folder, shown as `~`.
    pub home: Option<String>,
    /// Project folders besides those the input names under root keys.
    pub roots: BTreeSet<String>,
    pub zone: Zone,
}

/// A row under a report (a chapter, prompt or statistics bucket), keys in column order.
pub type Row = JsonMap;

/// What a format renders: the report, its rows (redacted like the report) and the options that
/// made them.
#[derive(Clone, Debug, PartialEq)]
pub struct Export {
    pub report: Report,
    pub rows: Vec<Row>,
    pub options: Options,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// A single-chart format found no chart with data (the API answers 404).
    #[error("this export has no chart with data")]
    NoChart,
    /// Asked for text from a binary format.
    #[error("{} is not a text format", .0.name())]
    NotText(Format),
    /// The redacted input no longer fits its type: a redaction rule dropped a required field (a bug).
    #[error("export input: {0}")]
    Input(String),
    /// Drawing a PNG failed.
    #[error("raster: {0}")]
    Raster(String),
    /// Reading the data to export failed.
    #[error(transparent)]
    Store(#[from] gitprompt_core::Error),
}

impl From<ExportError> for gitprompt_core::Error {
    /// `NoChart` → `NotFound`; `NotText` → `Invalid`; a store error as is; the rest → `Storage`.
    fn from(_error: ExportError) -> Self {
        todo!()
    }
}

/// Redact `input` once ([`redact::redact`] on its JSON form; for an Overview the repo bucket keys
/// count as project folders too), then build its report and rows.
pub fn prepare(_input: &ExportInput, _options: &Options, _context: &Context) -> Result<Export, ExportError> {
    todo!()
}

/// The export as a file's bytes. `chart`: the chart a single-chart format draws ([`charts::pick`]).
pub fn render(_export: &Export, _format: Format, _chart: Option<&str>) -> Result<Vec<u8>, ExportError> {
    todo!()
}

/// [`render`] for a text format; `ExportError::NotText` for `png`.
pub fn render_text(_export: &Export, _format: Format, _chart: Option<&str>) -> Result<String, ExportError> {
    todo!()
}

/// What to export from a store.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// A session by id.
    Session {
        id: String,
    },
    /// A project by its project id.
    Project {
        id: String,
    },
    Overview {
        filter: PromptFilter,
    },
    Prompts {
        filter: PromptFilter,
    },
}

/// Collect a request's data from the store (stories via gitprompt-insights, statistics, the
/// newest [`MAX_PROMPTS`] prompts with their facts, raw payloads only when `with_raw`).
/// `generated_at`: the caller's clock, ISO-8601. `Error::NotFound` for an unknown session/project.
pub fn collect(_store: &Store, _request: &Request, _with_raw: bool, _generated_at: &str) -> Result<ExportInput, ExportError> {
    todo!()
}

/// [`collect`] then [`prepare`].
pub fn build_from_store(
    _store: &Store,
    _request: &Request,
    _options: &Options,
    _context: &Context,
    _generated_at: &str,
) -> Result<Export, ExportError> {
    todo!()
}
