//! Exports: what to render, in which format, with which redactions, and the rendered file.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Session,
    Project,
    Overview,
    Prompts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Md,
    #[default]
    Html,
    Csv,
    Json,
    Svg,
    /// One chart as a PNG, rendered natively (2× the SVG size).
    Png,
}

/// Export options as query parameters; each defaults to the most private choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportQuery {
    /// session: its id; project: its project id. Required for those kinds (else 422).
    pub id: Option<String>,
    pub format: ExportFormat,
    /// svg: the chart id; unknown or none = the first chart.
    pub chart: Option<String>,
    pub download: bool,
    pub prompt_text: bool,
    pub relative_paths: bool,
    pub drop_identity: bool,
    pub drop_raw: bool,
}

impl Default for ExportQuery {
    fn default() -> Self {
        ExportQuery {
            id: None,
            format: ExportFormat::Html,
            chart: None,
            download: false,
            prompt_text: false,
            relative_paths: true,
            drop_identity: true,
            drop_raw: true,
        }
    }
}

/// A rendered export. HTTP: `Content-Type: media_type`, `Content-Disposition: <inline|attachment>;
/// filename="<filename>"`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, and for
/// HTML `Content-Security-Policy: <csp>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportFile {
    pub media_type: String,
    /// `gitprompt-<kind>[-<title slug>]-<local day of the app clock>.<ext>`, ASCII.
    pub filename: String,
    pub attachment: bool,
    pub csp: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ChartInfo {
    pub id: String,
    pub title: String,
}

impl From<ExportKind> for gitprompt_export::Kind {
    fn from(kind: ExportKind) -> Self {
        match kind {
            ExportKind::Session => gitprompt_export::Kind::Session,
            ExportKind::Project => gitprompt_export::Kind::Project,
            ExportKind::Overview => gitprompt_export::Kind::Overview,
            ExportKind::Prompts => gitprompt_export::Kind::Prompts,
        }
    }
}

impl From<ExportFormat> for gitprompt_export::Format {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::Md => gitprompt_export::Format::Markdown,
            ExportFormat::Html => gitprompt_export::Format::Html,
            ExportFormat::Csv => gitprompt_export::Format::Csv,
            ExportFormat::Json => gitprompt_export::Format::Json,
            ExportFormat::Svg => gitprompt_export::Format::Svg,
            ExportFormat::Png => gitprompt_export::Format::Png,
        }
    }
}

impl ExportQuery {
    /// The redaction options of the export.
    pub fn options(&self) -> gitprompt_export::Options {
        gitprompt_export::Options {
            prompt_text: self.prompt_text,
            relative_paths: self.relative_paths,
            drop_identity: self.drop_identity,
            drop_raw: self.drop_raw,
        }
    }
}
