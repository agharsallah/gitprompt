//! Recent captures: the newest prompt of each connector and surface, with how it was attributed
//! (session, project, model, surface) and the values the surface rules read. `gitprompt check` and
//! the Connectors page show it, so you can type one prompt in each app and see whether it arrived,
//! and where GitPrompt put it.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::spec::SurfaceRef;
use gitprompt_core::{Json, Result};
use gitprompt_store::stats::Latest;
use gitprompt_store::{EventRow, PromptRow, Store};

use crate::Specs;
use crate::facts::{Connector, connector};
use crate::stories::folder_name;
use crate::surface::signals;
use crate::text::text_of;

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
pub fn recent(store: &Store, specs: &Specs) -> Result<Vec<RecentCapture>> {
    let latest = store.latest()?;
    let ids: Vec<i64> = latest.iter().map(|row| row.id).collect();
    let raws: HashMap<i64, PromptRow> = store.prompts(&ids)?.into_iter().map(|row| (row.id, row)).collect();
    let turns: Vec<String> = raws.values().filter_map(prompt_id).map(String::from).collect();
    let mut events: HashMap<String, Vec<EventRow>> = HashMap::new();
    for event in store.events_for_prompts(&turns)? {
        if let Some(pid) = event.prompt_id.clone() {
            events.entry(pid).or_default().push(event);
        }
    }
    Ok(latest
        .into_iter()
        .map(|row| {
            let raw = raws.get(&row.id);
            capture(row, raw, &events, specs)
        })
        .collect())
}

/// The prompt id a stored prompt's turn events carry.
fn prompt_id(row: &PromptRow) -> Option<&str> {
    row.fields.get("prompt_id").and_then(Json::as_str).filter(|pid| !pid.is_empty())
}

fn capture(row: Latest, raw: Option<&PromptRow>, events: &HashMap<String, Vec<EventRow>>, specs: &Specs) -> RecentCapture {
    let spec = specs.get(&row.source).map(AsRef::as_ref);
    let turn = raw.and_then(prompt_id).and_then(|pid| events.get(pid)).map_or(&[][..], Vec::as_slice);
    let project = row.repo.clone().filter(|repo| !repo.is_empty()).map(|repo| {
        let name = row.repo_name.clone().filter(|name| !name.is_empty()).unwrap_or_else(|| folder_name(&repo));
        CaptureProject { repo, name }
    });
    RecentCapture {
        id: row.id,
        connector: connector(&row.source, spec),
        surface: spec.and_then(|spec| spec.surface_info(row.surface.as_deref())),
        project,
        worktree: raw.and_then(|r| r.context.get("git_worktree_root")).and_then(text_of),
        signals: raw.map(|r| signals(r, turn, spec)).unwrap_or_default(),
        received_at: row.received_at,
        session: row.session,
        title: row.title,
        branch: row.branch,
        model: row.model,
        origin: row.origin,
        prompts: row.prompts,
    }
}
