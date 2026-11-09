//! Recent captures: the newest prompt of each connector and surface, with how it was attributed
//! (session, project, model, surface) and the values the surface rules read. `gitprompt check` and
//! the Connectors page show it, so you can type one prompt in each app and see whether it arrived,
//! and where GitPrompt put it.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::spec::SurfaceRef;
use gitprompt_core::{Json, Result};
use gitprompt_store::Store;

use crate::Specs;
use crate::facts::Connector;

/// The project a capture was put in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureProject {
    pub repo: String,
    /// `repo_name` (owner/name), else the last segment of `repo` (trailing `/` ignored).
    pub name: String,
}

/// The newest prompt of one connector and surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentCapture {
    pub id: i64,
    pub connector: Connector,
    /// `None` when the surface is unknown or the source has no spec.
    pub surface: Option<SurfaceRef>,
    pub received_at: String,
    pub session: Option<String>,
    pub title: Option<String>,
    /// `None` without a repo.
    pub project: Option<CaptureProject>,
    pub branch: Option<String>,
    /// `context.git_worktree_root` of the stored prompt: the linked worktree it was typed in.
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub origin: String,
    /// Prompts of this connector and surface.
    pub prompts: i64,
    /// What the surface rules read ([`crate::surface::signals`]) from the prompt and its turn.
    pub signals: IndexMap<String, Json>,
}

/// One entry per (connector, surface), newest first (`Store::latest`), from the prompt index: call
/// [`crate::indexer::Indexer::ensure`] first.
pub fn recent(_store: &Store, _specs: &Specs) -> Result<Vec<RecentCapture>> {
    todo!()
}
