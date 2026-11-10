//! Exports (plan E5, ADR-0018): a session, a project, the Overview or a prompt selection as a file
//! you send yourself. Nothing leaves the machine; nothing here does I/O except [`collect`] (and its
//! `_with` / [`build_from_store`] forms), which reads the store and runs `git log`.
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
use gitprompt_insights::Specs;
use gitprompt_store::{PromptFilter, Store};

pub mod build;
pub mod charts;
mod collect;
mod escape;
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
        match self {
            Kind::Session => "session",
            Kind::Project => "project",
            Kind::Overview => "overview",
            Kind::Prompts => "prompts",
        }
    }

    /// The kind of a path segment; `None` for an unknown one (the API answers 422).
    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.as_str() == name)
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
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::NoChart => gitprompt_core::Error::NotFound(error.to_string()),
            ExportError::NotText(_) => gitprompt_core::Error::Invalid(error.to_string()),
            ExportError::Store(inner) => inner,
            ExportError::Input(_) | ExportError::Raster(_) => gitprompt_core::Error::Storage(error.to_string()),
        }
    }
}

/// Redact `input` once ([`redact::redact`] on its JSON form; for an Overview the repo bucket keys
/// count as project folders too), then build its report and rows.
pub fn prepare(input: &ExportInput, options: &Options, context: &Context) -> Result<Export, ExportError> {
    let mut roots = context.roots.clone();
    if let ExportInput::Overview(overview) = input {
        roots.extend(overview.groups.repo.iter().filter_map(|bucket| bucket.key.as_deref().and_then(redact::as_root)));
    }
    let json = serde_json::to_value(input).map_err(|e| ExportError::Input(e.to_string()))?;
    let redacted = redact::redact(&json, options, context.home.as_deref(), &roots);
    let clean: ExportInput = serde_json::from_value(redacted).map_err(|e| ExportError::Input(e.to_string()))?;
    let (report, rows) = build::build(&clean, options, context.zone);
    Ok(Export { report, rows, options: *options })
}

/// A rendered file: text for every format but `png`.
enum Body {
    Text(String),
    Binary(Vec<u8>),
}

/// The one place a format is dispatched to its renderer: a new format is a new module and one arm.
fn body(export: &Export, format: Format, chart: Option<&str>) -> Result<Body, ExportError> {
    Ok(match format {
        Format::Markdown => Body::Text(markdown::markdown(&export.report)),
        Format::Html => Body::Text(html::html(&export.report)),
        Format::Csv => Body::Text(tabular::csv(export)),
        Format::Json => Body::Text(tabular::json(export)),
        Format::Svg => Body::Text(charts::svg_file(&export.report, chart)?),
        Format::Png => Body::Binary(charts::png(charts::pick(&export.report, chart)?, charts::PNG_SCALE)?),
    })
}

/// The export as a file's bytes. `chart`: the chart a single-chart format draws ([`charts::pick`]).
pub fn render(export: &Export, format: Format, chart: Option<&str>) -> Result<Vec<u8>, ExportError> {
    Ok(match body(export, format, chart)? {
        Body::Text(text) => text.into_bytes(),
        Body::Binary(bytes) => bytes,
    })
}

/// [`render`] for a text format; `ExportError::NotText` for `png`.
pub fn render_text(export: &Export, format: Format, chart: Option<&str>) -> Result<String, ExportError> {
    if !format.is_text() {
        return Err(ExportError::NotText(format));
    }
    match body(export, format, chart)? {
        Body::Text(text) => Ok(text),
        Body::Binary(_) => Err(ExportError::NotText(format)),
    }
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
///
/// Without source specs: a prompt's tool is named by its source id, and no surface is known. The
/// server, which has the specs, calls [`collect_with`].
pub fn collect(store: &Store, request: &Request, with_raw: bool, generated_at: &str) -> Result<ExportInput, ExportError> {
    collect_with(store, &Specs::new(), request, with_raw, generated_at)
}

/// [`collect`] with the source specs the facts and stories read (tool names and labels, surfaces,
/// whether a tool reports costs; each tool's kinds and the files it writes).
pub fn collect_with(
    store: &Store,
    specs: &Specs,
    request: &Request,
    with_raw: bool,
    generated_at: &str,
) -> Result<ExportInput, ExportError> {
    collect::collect(store, specs, request, with_raw, generated_at)
}

/// [`collect`] then [`prepare`]; stored payloads are only read when the options keep them.
pub fn build_from_store(
    store: &Store,
    request: &Request,
    options: &Options,
    context: &Context,
    generated_at: &str,
) -> Result<Export, ExportError> {
    build_from_store_with(store, &Specs::new(), request, options, context, generated_at)
}

/// [`collect_with`] then [`prepare`].
pub fn build_from_store_with(
    store: &Store,
    specs: &Specs,
    request: &Request,
    options: &Options,
    context: &Context,
    generated_at: &str,
) -> Result<Export, ExportError> {
    let input = collect_with(store, specs, request, !options.drop_raw, generated_at)?;
    prepare(&input, options, context)
}
