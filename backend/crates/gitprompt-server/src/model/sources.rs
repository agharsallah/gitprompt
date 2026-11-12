//! The Connectors page: each connector's facts, its spec, its recent captures, connect/disconnect.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::JsonMap;
use gitprompt_core::spec::{Capability, Join, Origin, SpecOrigin, SurfaceRef};
use gitprompt_insights::facts::Connector;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "SpecFile")]
pub struct SpecFileOut {
    pub name: String,
    pub path: String,
    pub origin: SpecOrigin,
    pub overrides: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "SpecProblem")]
pub struct SpecProblemOut {
    pub file: SpecFileOut,
    pub error: String,
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CapturedSurface {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// What of this app reaches the connector (spec `captures`).
    pub note: String,
}

/// Activity window of `SourceOut::hourly`: prompts + events per UTC hour, the current hour last.
pub const HOURS: usize = 24;

/// One connector with the raw facts the dashboard derives its state from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "Source")]
pub struct SourceOut {
    pub id: String,
    pub name: String,
    pub assistant: String,
    pub part_of: Option<String>,
    pub layer: String,
    pub summary: String,
    /// `false`: turned off in GitPrompt (preference `disabled_sources`).
    pub enabled: bool,
    pub detected: bool,
    pub version: Option<String>,
    pub connected: bool,
    pub outdated: bool,
    pub config_file: String,
    pub terms: String,
    pub after_connect: String,
    pub blocks: Vec<String>,
    pub warnings: Vec<String>,
    pub total: u64,
    pub unmapped: u64,
    pub events: u64,
    pub last_at: Option<String>,
    /// [`HOURS`] counts, oldest first, the current UTC hour (by the app clock) last.
    pub hourly: Vec<u64>,
    pub collects: Vec<String>,
    pub not_stored: IndexMap<String, String>,
    pub not_collected: IndexMap<String, String>,
    pub capabilities: Vec<Capability>,
    pub needs_version: IndexMap<Capability, String>,
    pub join: Join,
    pub captures: Vec<CapturedSurface>,
    pub cannot_capture: IndexMap<String, String>,
    pub spec: SpecFileOut,
    pub spec_problem: Option<SpecProblemOut>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CaptureProject")]
pub struct ProjectRef {
    pub repo: String,
    pub name: String,
}

/// The newest prompt of one connector and surface (`GET /api/sources/recent`), newest first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct RecentCapture {
    pub id: i64,
    pub connector: Connector,
    pub surface: Option<SurfaceRef>,
    pub received_at: String,
    pub session: Option<String>,
    pub title: Option<String>,
    pub project: Option<ProjectRef>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub origin: Origin,
    pub prompts: i64,
    /// What the surface rules read, e.g. `{"app.entrypoint": "cli"}`.
    pub signals: JsonMap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecText {
    pub file: SpecFileOut,
    pub text: String,
}

/// A connect/disconnect (or its dry run): the unified diff of the tool's config file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Change {
    pub file: String,
    /// Empty when there is nothing to change.
    pub diff: String,
    pub applied: bool,
    pub backup: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Enabled {
    pub enabled: bool,
}

impl From<gitprompt_core::spec::SpecFile> for SpecFileOut {
    fn from(f: gitprompt_core::spec::SpecFile) -> Self {
        SpecFileOut { name: f.name, path: f.path, origin: f.origin, overrides: f.overrides }
    }
}

impl From<gitprompt_core::spec::SpecProblem> for SpecProblemOut {
    fn from(p: gitprompt_core::spec::SpecProblem) -> Self {
        SpecProblemOut { file: p.file.into(), error: p.error, fallback: p.fallback }
    }
}

impl From<gitprompt_capture::Change> for Change {
    fn from(c: gitprompt_capture::Change) -> Self {
        Change { file: c.file, diff: c.diff, applied: c.applied, backup: c.backup }
    }
}

impl From<gitprompt_insights::captures::RecentCapture> for RecentCapture {
    fn from(r: gitprompt_insights::captures::RecentCapture) -> Self {
        RecentCapture {
            id: r.id,
            connector: r.connector,
            surface: r.surface,
            received_at: r.received_at,
            session: r.session,
            title: r.title,
            project: r.project.map(|p| ProjectRef { repo: p.repo, name: p.name }),
            branch: r.branch,
            worktree: r.worktree,
            model: r.model,
            origin: gitprompt_insights::origin::parse(&r.origin).unwrap_or(Origin::Unknown),
            prompts: r.prompts,
            signals: r.signals.into_iter().collect(),
        }
    }
}
