//! Stories (a session's, a project's, a prompt's versions) and full-text search, all read from
//! the prompt timeline with its facts attached.

use std::path::Path;

use gitprompt_core::Error;
use gitprompt_core::git::Commit;
use gitprompt_core::spec::SpecSet;
use gitprompt_insights::facts::{FactRow, attach};
use gitprompt_insights::snippets::{WIDTH, highlight};
use gitprompt_insights::stories::{self, TurnTools, USER_ORIGINS};
use gitprompt_store::filter::PromptFilter;

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::{CommitQuery, Lineage, ProjectCommits, ProjectStory, PromptQuery, SearchHit, SearchResults, SessionStory};

/// Characters of a subagent brief in a session story (Python's `timeline.turn_events` default).
const BRIEF_CHARS: usize = 600;
/// Prompts compared when looking for the versions of one, newest first.
const LINEAGE_CANDIDATES: usize = 5000;

impl Inner {
    fn timeline(&self, set: &SpecSet, filter: &PromptFilter, limit: Option<usize>, newest_first: bool) -> ApiResult<Vec<FactRow>> {
        Ok(attach(self.store.timeline(filter, limit, newest_first)?, &set.specs))
    }

    /// Commits in the story's repository while it lasted (one git call); `[]` without one.
    fn log(&self, rows: &[FactRow]) -> Vec<Commit> {
        self.log_in(rows, stories::log_window(rows))
    }

    /// Commits in the repository of `rows` within `window` (one git call); `[]` without either.
    pub(super) fn log_in(&self, rows: &[FactRow], window: Option<(String, String)>) -> Vec<Commit> {
        let Some((since, until)) = window else { return Vec::new() };
        let filled = |v: &Option<String>| v.as_deref().is_some_and(|v| !v.is_empty());
        let repo =
            rows.iter().find(|r| filled(&r.prompt.row.branch) && filled(&r.prompt.row.repo)).and_then(|r| r.prompt.row.repo.as_deref());
        repo.map_or_else(Vec::new, |repo| self.options.git.history(Path::new(repo), &since, &until))
    }

    /// A project's story (its typed prompts in phases, each with its commits); not found as
    /// [`Inner::project_rows`] says.
    pub(super) fn project_story(&self, project: &str) -> ApiResult<ProjectStory> {
        let (set, _) = self.indexed()?;
        let ProjectRows { repo, rows } = self.project_rows(&set, project)?;
        let typed: Vec<i64> = rows.iter().filter(|r| stories::is_user(r)).map(|r| r.prompt.row.id).collect();
        let system = i64::try_from(rows.len() - typed.len()).unwrap_or(i64::MAX);
        let commits = self.log(&rows);
        let tool_kinds = self.store.tool_kinds(&typed)?;
        Ok(stories::project_story(&repo, &rows, &tool_kinds, &commits, system))
    }

    /// The repo a project id names and its prompts of every origin, oldest first (the ones you did
    /// not type carry a prompt's work on, `stories::commit_traces`); not found when the id doesn't
    /// decode or the project has no prompt.
    fn project_rows(&self, set: &SpecSet, project: &str) -> ApiResult<ProjectRows> {
        let not_found = || ApiError::from(Error::NotFound("project not found".into()));
        let repo = stories::project_repo(project).ok_or_else(not_found)?;
        self.repo_rows(set, repo)
    }

    /// [`Self::project_rows`] of the repo itself (a git root or folder).
    pub(super) fn repo_rows(&self, set: &SpecSet, repo: String) -> ApiResult<ProjectRows> {
        let not_found = || ApiError::from(Error::NotFound("project not found".into()));
        let rows = self.timeline(set, &PromptFilter { repo: Some(repo.clone()), ..PromptFilter::default() }, None, false)?;
        if rows.is_empty() {
            return Err(not_found());
        }
        Ok(ProjectRows { repo, rows })
    }
}

/// A project's prompts of every origin ([`Inner::project_rows`]).
pub(super) struct ProjectRows {
    pub(super) repo: String,
    pub(super) rows: Vec<FactRow>,
}

impl Service {
    pub async fn session_story(&self, session: &str) -> ApiResult<SessionStory> {
        let session = session.to_owned();
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let filter = PromptFilter { session: vec![session.clone()], ..PromptFilter::default() };
            let rows = inner.timeline(&set, &filter, None, false)?;
            let first = rows.first().ok_or_else(|| Error::NotFound("session not found".into()))?;
            let user: Vec<FactRow> = rows.iter().filter(|r| stories::is_user(r)).cloned().collect();
            let turns: Vec<String> = rows.iter().filter_map(|r| r.prompt.row.prompt_id.clone()).collect();
            let events = store.turn_events(&turns, BRIEF_CHARS)?;
            let commits = inner.log(&rows);
            let tool_kinds = store.tool_kinds(&user.iter().map(|r| r.prompt.row.id).collect::<Vec<_>>())?;
            let spec = set.get(&first.prompt.row.source);
            let (none_kinds, none_files) = Default::default();
            let tools = TurnTools {
                tool_kinds: &tool_kinds,
                events: &events,
                kinds: spec.map_or(&none_kinds, |s| &s.tool_kinds),
                tool_files: spec.map_or(&none_files, |s| &s.tool_files),
            };
            let story = stories::session_story(&session, &rows, tools, &commits);
            let present = inner.presence_of_session(&set, &session)?;
            Ok(SessionStory {
                name: inner.name_of_session(&session)?,
                resume: present.resume,
                last_event_at: present.last_event_at,
                ..story
            })
        })
        .await
    }

    pub async fn project_story(&self, project: &str) -> ApiResult<ProjectStory> {
        let project = project.to_owned();
        self.run(move |inner| inner.project_story(&project)).await
    }

    /// A project's commits since you started on it ([`stories::trace_window`]), each with the prompt
    /// behind it and its page on the forge, matched against `query.q` and grouped by `query.group`.
    pub async fn project_commits(&self, project: &str, query: CommitQuery) -> ApiResult<ProjectCommits> {
        let project = project.to_owned();
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let ProjectRows { repo, rows } = inner.project_rows(&set, &project)?;
            let commits = inner.log_in(&rows, stories::trace_window(&rows, &inner.now().to_string()));
            let remote = inner.remote_of(&repo);
            Ok(stories::project_commits(&repo, &rows, &commits, &query, remote.as_deref()))
        })
        .await
    }

    pub async fn lineage(&self, id: i64) -> ApiResult<Lineage> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let target = inner.store.timeline_one(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let target = attach(vec![target], &set.specs).pop().ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let filter = PromptFilter { origin: USER_ORIGINS.to_vec(), ..PromptFilter::default() };
            let candidates = inner.timeline(&set, &filter, Some(LINEAGE_CANDIDATES), true)?;
            Ok(stories::prompt_lineage(&target, &candidates))
        })
        .await
    }

    /// Every word of `filter.q` in the typed text, newest first, with snippets; `total` ignores
    /// `limit` and `before`.
    pub async fn search(&self, query: PromptQuery) -> ApiResult<SearchResults> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let rows = inner.timeline(&set, &query.filter, Some(query.limit), true)?;
            let total = inner.store.count(&PromptFilter { before: None, ..query.filter.clone() })?;
            let hits = rows.iter().map(|r| search_hit(r, query.filter.q.as_deref())).collect();
            Ok(SearchResults { total: u64::try_from(total).unwrap_or(0), hits })
        })
        .await
    }
}

/// One search match, its typed text cut around the words of `q`.
fn search_hit(r: &FactRow, q: Option<&str>) -> SearchHit {
    let row = &r.prompt.row;
    SearchHit {
        id: row.id,
        received_at: row.received_at.clone(),
        session: row.session.clone(),
        title: row.title.clone(),
        project: stories::project_of(std::slice::from_ref(r)),
        branch: row.branch.clone(),
        origin: row.origin.clone(),
        outcome: r.prompt.outcome,
        cost_usd: row.usage.cost_usd,
        facts: r.facts.clone(),
        snippet: highlight(row.typed_text.as_deref(), q, WIDTH),
    }
}
