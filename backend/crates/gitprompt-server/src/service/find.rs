//! Find: the best prompts, sessions and projects for one query. The store answers each kind with
//! reads that already exist (the prompt filter on the typed-text index, statistics grouped by
//! session and by repo); `gitprompt_insights::find` ranks them. Sessions carry their names and
//! default labels (ticket 09) first, so find matches what the dashboard calls them.

use gitprompt_core::Result;
use gitprompt_insights::find::{Kind, matches, project_hits, prompt_hits, prompt_hits_up_to, session_hits};
use gitprompt_insights::stories::{USER_ORIGINS, folder_name};
use gitprompt_store::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, Bucket, GroupBy, Order};
use gitprompt_store::store::NameKind;
use gitprompt_store::timeline::IndexedPrompt;

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::{FindHit, FindQuery, FindResults, FindTarget};

/// Candidates read per kind before ranking: the most recent ones. Prompts are matched by the store
/// over all time first, so only a finished older prompt behind 200 newer matches is missed. Sessions
/// and projects are matched here, so one older than the pool is found only by a name you gave it
/// (`named_beyond`) or through one of its prompts.
const POOL: usize = 200;

/// A prompt that matched, as the terminal tells it: the hit plus where it was typed and the whole
/// text (the palette's hit carries only a marked snippet).
#[derive(Clone, Debug, PartialEq)]
pub struct PromptMatch {
    pub hit: FindHit,
    /// The folder name of the project (git root or working folder), when the tool said.
    pub project: Option<String>,
    /// The tool's id, e.g. `claude-code`.
    pub source: String,
    /// What you typed, in full.
    pub text: String,
}

impl Inner {
    /// The prompts whose typed text matches `q` (every prompt without words), newest first, from
    /// the repo `repo` only when given.
    fn prompt_rows(&self, q: &str, repo: Option<String>) -> Result<Vec<IndexedPrompt>> {
        let filter = PromptFilter {
            q: Some(q.to_owned()).filter(|q| !q.trim().is_empty()),
            origin: USER_ORIGINS.to_vec(),
            repo,
            ..PromptFilter::default()
        };
        self.store.timeline(&filter, Some(POOL), true)
    }

    /// The repos (git roots or folders) that `project` names: the path itself, the folder's name
    /// or the project's name, ignoring case.
    fn repos_named(&self, project: &str) -> Result<Vec<String>> {
        let named = |text: &str| text.eq_ignore_ascii_case(project);
        let repos = self.recent_groups(GroupBy::Repo)?.into_iter();
        Ok(repos
            .filter_map(|g| g.key.filter(|repo| named(repo) || named(&folder_name(repo)) || g.detail.as_deref().is_some_and(named)))
            .collect())
    }

    /// The most recent groups of a grouping, newest activity first.
    fn recent_groups(&self, group_by: GroupBy) -> Result<Vec<Bucket>> {
        let query = AggregateQuery { group_by: Some(group_by), order: Some(Order::LastAt), limit: POOL, ..AggregateQuery::default() };
        Ok(self.store.aggregate(&query)?.groups)
    }

    /// The sessions you named whose name matches `q` and that are not among `have`: a name you
    /// typed finds its session however many newer ones there are.
    fn named_beyond(&self, q: &str, have: &[Bucket]) -> Result<Vec<Bucket>> {
        let known = |id: &str| have.iter().any(|group| group.key.as_deref() == Some(id));
        let names = self.store.all_names(NameKind::Session)?;
        let session: Vec<String> =
            names.into_iter().filter(|(id, name)| matches(name, q) && !known(id)).map(|(id, _)| id).take(POOL).collect();
        if session.is_empty() {
            return Ok(Vec::new());
        }
        let limit = session.len();
        let filter = PromptFilter { session, ..PromptFilter::default() };
        let query =
            AggregateQuery { filter, group_by: Some(GroupBy::Session), order: Some(Order::LastAt), limit, ..AggregateQuery::default() };
        Ok(self.store.aggregate(&query)?.groups)
    }
}

impl Service {
    /// The best matches of each requested kind, at most `query.limit` each; kinds not asked for
    /// stay empty.
    pub async fn find(&self, query: FindQuery) -> ApiResult<FindResults> {
        self.run(move |inner| {
            inner.indexed()?;
            let wants = |kind| query.kinds.contains(&kind);
            let (q, limit) = (query.q.as_str(), query.limit);
            let mut found = FindResults::default();
            if wants(Kind::Prompts) {
                found.prompts = prompt_hits(&inner.prompt_rows(q, None)?, q, limit);
            }
            if wants(Kind::Sessions) {
                let mut sessions = inner.recent_groups(GroupBy::Session)?;
                let older = inner.named_beyond(q, &sessions)?;
                sessions.extend(older);
                inner.name_sessions(&mut sessions)?;
                found.sessions = session_hits(&sessions, q, limit);
            }
            if wants(Kind::Projects) {
                found.projects = project_hits(&inner.recent_groups(GroupBy::Repo)?, q, limit);
            }
            Ok(found)
        })
        .await
    }
}

impl Service {
    /// The prompts of `gitprompt find`: the ranking of [`Self::find`], over the prompts of
    /// `project` when given (a folder name, a path or the project's name; none that matches gives
    /// none), up to `limit` (not held to the palette's few), each with its project and full text.
    pub async fn find_prompts(&self, q: &str, project: Option<&str>, limit: usize) -> ApiResult<Vec<PromptMatch>> {
        let (q, project) = (q.to_owned(), project.map(str::to_owned));
        self.run(move |inner| {
            inner.indexed()?;
            let repos = match &project {
                None => vec![None],
                Some(project) => inner.repos_named(project)?.into_iter().map(Some).collect(),
            };
            let rows = repos.into_iter().map(|repo| inner.prompt_rows(&q, repo)).collect::<Result<Vec<_>>>()?.concat();
            Ok(prompt_hits_up_to(&rows, &q, limit).into_iter().filter_map(|hit| prompt_match(&rows, hit)).collect())
        })
        .await
    }
}

/// The hit joined with the row it came from.
fn prompt_match(rows: &[IndexedPrompt], hit: FindHit) -> Option<PromptMatch> {
    let FindTarget::Prompt { id } = hit.target else { return None };
    let row = &rows.iter().find(|r| r.row.id == id)?.row;
    let project = row.repo.as_deref().map(folder_name).filter(|name| !name.is_empty());
    Some(PromptMatch { project, source: row.source.clone(), text: row.typed_text.clone().unwrap_or_default(), hit })
}
