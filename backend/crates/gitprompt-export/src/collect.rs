//! Collecting what an export shows from the store (the only I/O of this crate): stories through
//! gitprompt-insights (with one `git log` per story), statistics, and a prompt selection with its
//! facts. Port of `_collect` in `api/routes/export.py` and of the story routes it calls.
//!
//! The insights types are turned into this crate's [`input`](crate::input) types through their
//! JSON form: both mirror the JSON the dashboard reads, so the conversion is the API contract.

use std::collections::BTreeMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::Serialize;
use serde::de::DeserializeOwned;

use gitprompt_core::{Error, Json};
use gitprompt_insights::Specs;
use gitprompt_insights::derivations;
use gitprompt_insights::facts::{self, FactRow};
use gitprompt_insights::stories::{self, TurnTools};
use gitprompt_store::stats::{AggregateQuery, GroupBy, Order};
use gitprompt_store::{PromptFilter, Store};

use crate::input::{ExportInput, OverviewExport, OverviewGroups, ProjectExport, PromptItem, PromptsExport, SessionExport};
use crate::{ExportError, GROUP_LIMIT, MAX_PROMPTS, Request};

/// Characters of a subagent brief read per turn event (Python's `turn_events` default).
const BRIEF_CHARS: usize = 600;

/// A request's data from the store, with facts from `specs`. `generated_at`: the caller's clock.
pub(crate) fn collect(
    store: &Store,
    specs: &Specs,
    request: &Request,
    with_raw: bool,
    generated_at: &str,
) -> Result<ExportInput, ExportError> {
    let generated_at = generated_at.to_string();
    Ok(match request {
        Request::Session { id } => ExportInput::Session(session(store, specs, id, generated_at)?),
        Request::Project { id } => ExportInput::Project(project(store, specs, id, generated_at)?),
        Request::Overview { filter } => ExportInput::Overview(overview(store, &without_paging(filter), generated_at)?),
        Request::Prompts { filter } => ExportInput::Prompts(prompts(store, specs, &without_paging(filter), with_raw, generated_at)?),
    })
}

/// The filter of a whole selection: a paging cursor is not part of it.
fn without_paging(filter: &PromptFilter) -> PromptFilter {
    PromptFilter { before: None, ..filter.clone() }
}

/// `value` as one of this crate's input types, through its JSON form.
fn convert<T: DeserializeOwned>(value: impl Serialize) -> Result<T, ExportError> {
    serde_json::to_value(value).and_then(serde_json::from_value).map_err(|e| ExportError::Input(e.to_string()))
}

/// Commits in the story's repository while it lasted (one git call), or none without a repository
/// (a row with a branch names one).
fn log(rows: &[FactRow]) -> Vec<gitprompt_core::git::Commit> {
    let repo =
        rows.iter().find(|r| r.prompt.row.branch.as_deref().is_some_and(|b| !b.is_empty())).and_then(|r| r.prompt.row.repo.as_deref());
    match (repo.filter(|r| !r.is_empty()), stories::log_window(rows)) {
        (Some(repo), Some((since, until))) => gitprompt_core::git::history(Path::new(repo), &since, &until),
        _ => Vec::new(),
    }
}

fn session(store: &Store, specs: &Specs, id: &str, generated_at: String) -> Result<SessionExport, ExportError> {
    let filter = PromptFilter { session: vec![id.to_string()], ..PromptFilter::default() };
    let rows = facts::attach(store.timeline(&filter, None, false)?, specs);
    let Some(first) = rows.first() else { return Err(Error::NotFound("session not found".into()).into()) };
    let user: Vec<FactRow> = rows.iter().filter(|r| stories::is_user(r)).cloned().collect();
    let turn_ids: Vec<String> = rows.iter().filter_map(|r| r.prompt.row.prompt_id.clone()).collect();
    let events = store.turn_events(&turn_ids, BRIEF_CHARS)?;
    let tool_kinds = store.tool_kinds(&user.iter().map(|r| r.prompt.row.id).collect::<Vec<_>>())?;
    let spec = specs.get(&first.prompt.row.source);
    let (no_kinds, no_files) = (IndexMap::new(), IndexMap::new());
    let turns = TurnTools {
        tool_kinds: &tool_kinds,
        events: &events,
        kinds: spec.map_or(&no_kinds, |s| &s.tool_kinds),
        tool_files: spec.map_or(&no_files, |s| &s.tool_files),
    };
    let story = stories::session_story(id, &rows, turns, &log(&user));
    let environment = derivations::session(&store.events_for_session(id)?)
        .environment
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                Json::String(text) => text,
                other => other.to_string(),
            };
            (key, value)
        })
        .collect();
    Ok(SessionExport { generated_at, story: convert(story)?, environment })
}

fn project(store: &Store, specs: &Specs, id: &str, generated_at: String) -> Result<ProjectExport, ExportError> {
    let not_found = || ExportError::Store(Error::NotFound("project not found".into()));
    let repo = stories::project_repo(id).ok_or_else(not_found)?;
    let all = PromptFilter { repo: Some(repo.clone()), ..PromptFilter::default() };
    let typed = PromptFilter { origin: stories::USER_ORIGINS.to_vec(), ..all.clone() };
    let rows = facts::attach(store.timeline(&typed, None, false)?, specs);
    let count = store.count(&all)?;
    if rows.is_empty() && count == 0 {
        return Err(not_found());
    }
    let system = count - i64::try_from(rows.len()).unwrap_or(i64::MAX);
    let tool_kinds = store.tool_kinds(&rows.iter().map(|r| r.prompt.row.id).collect::<Vec<_>>())?;
    let story = stories::project_story(&repo, &rows, &tool_kinds, &log(&rows), system);
    Ok(ProjectExport { generated_at, story: convert(story)? })
}

/// The groupings of an Overview, as [`crate::build::OVERVIEW_GROUPS`] names them.
const GROUPS: [(&str, GroupBy); 7] = [
    ("day", GroupBy::Day),
    ("origin", GroupBy::Origin),
    ("model", GroupBy::Model),
    ("tool_kind", GroupBy::ToolKind),
    ("outcome", GroupBy::Outcome),
    ("repo", GroupBy::Repo),
    ("command", GroupBy::Command),
];

fn overview(store: &Store, filter: &PromptFilter, generated_at: String) -> Result<OverviewExport, ExportError> {
    let mut groups: BTreeMap<&str, Vec<gitprompt_store::stats::Bucket>> = BTreeMap::new();
    for (name, group_by) in GROUPS {
        // Projects and commands by prompts; the rest in the store's own order (days by key).
        let order = matches!(group_by, GroupBy::Repo | GroupBy::Command).then_some(Order::Prompts);
        let query =
            AggregateQuery { filter: filter.clone(), group_by: Some(group_by), order, limit: GROUP_LIMIT, ..AggregateQuery::default() };
        groups.insert(name, store.aggregate(&query)?.groups);
    }
    let totals = store.aggregate(&AggregateQuery { filter: filter.clone(), ..AggregateQuery::default() })?.totals;
    let mut take = |name: &str| groups.remove(name).unwrap_or_default();
    let groups = OverviewGroups {
        day: take("day"),
        origin: take("origin"),
        model: take("model"),
        tool_kind: take("tool_kind"),
        outcome: take("outcome"),
        repo: take("repo"),
        command: take("command"),
    };
    Ok(OverviewExport { generated_at, filters: filter.clone(), totals, groups })
}

fn prompts(
    store: &Store,
    specs: &Specs,
    filter: &PromptFilter,
    with_raw: bool,
    generated_at: String,
) -> Result<PromptsExport, ExportError> {
    let rows = facts::attach(store.timeline(filter, Some(MAX_PROMPTS), true)?, specs);
    let mut raw: BTreeMap<i64, Json> =
        if with_raw { store.search(filter, MAX_PROMPTS)?.into_iter().map(|p| (p.row.id, p.row.raw)).collect() } else { BTreeMap::new() };
    let prompts = rows
        .into_iter()
        .map(|row| {
            let id = row.prompt.row.id;
            Ok(PromptItem { prompt: row.prompt, facts: row.facts.map(convert).transpose()?, raw: raw.remove(&id) })
        })
        .collect::<Result<Vec<_>, ExportError>>()?;
    Ok(PromptsExport { generated_at, filters: filter.clone(), total: store.count(filter)?, prompts })
}
