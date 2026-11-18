//! A commit by its hash, from any front end (`GET /api/commits/{hash}`, the CLI's `why`): the
//! projects' commits are read as their Commits tab reads them (`stories::project_traces`), so a hash
//! names the same commit, traced to the same prompt, everywhere.

use std::path::Path;

use gitprompt_core::Error;
use gitprompt_core::spec::SpecSet;
use gitprompt_insights::stories::{self, MIN_HASH, ProjectTraces};
use gitprompt_store::stats::{AggregateQuery, GroupBy, Order};

use super::stories::ProjectRows;
use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::CommitLookup;

/// Projects whose commits a hash is looked up in: the most recently active ones.
const LOOKUP_PROJECTS: usize = 200;
/// The longest hash (SHA-256 object names).
const MAX_HASH: usize = 64;

impl Inner {
    /// The repos (git roots or folders) of the `limit` most recently active projects, newest first.
    fn recent_repos(&self, limit: usize) -> ApiResult<Vec<String>> {
        let query = AggregateQuery { group_by: Some(GroupBy::Repo), order: Some(Order::LastAt), limit, ..AggregateQuery::default() };
        Ok(self.store.aggregate(&query)?.groups.into_iter().filter_map(|g| g.key.filter(|repo| !repo.is_empty())).collect())
    }

    /// The traced commits of the `limit` most recently active projects; a project that is not a git
    /// repository has none.
    pub(super) fn recent_traces(&self, set: &SpecSet, limit: usize) -> ApiResult<Vec<ProjectTraces>> {
        self.recent_repos(limit)?.into_iter().map(|repo| self.traces_of(set, repo)).collect()
    }

    /// A project's commits while you worked on it, each traced: the read behind its Commits tab.
    fn traces_of(&self, set: &SpecSet, repo: String) -> ApiResult<ProjectTraces> {
        let ProjectRows { repo, rows, .. } = self.repo_rows(set, repo)?;
        let commits = self.log_in(&rows, stories::trace_window(&rows));
        Ok(stories::project_traces(&repo, &rows, &commits))
    }

    /// The URL of the `origin` remote of the project at `repo`, credentials stripped (the git facts'
    /// `git_remote`).
    pub(super) fn remote_of(&self, repo: &str) -> Option<String> {
        let context = self.options.git.context(Some(Path::new(repo)));
        context.get("git_remote").and_then(|remote| remote.as_str()).map(str::to_owned)
    }
}

impl Service {
    /// The commit `hash` names (a full hash or a prefix of at least [`MIN_HASH`] hex digits) among
    /// the commits of the most recently active projects, with the prompt behind it and its page on
    /// the forge; every candidate when a prefix names several. 400 for a hash that is not one, 404
    /// when no project's commits have it.
    pub async fn commit(&self, hash: &str) -> ApiResult<CommitLookup> {
        let hash = hash.trim().to_owned();
        if !(MIN_HASH..=MAX_HASH).contains(&hash.len()) || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Invalid(format!("{hash:?} is not a commit hash ({MIN_HASH} to {MAX_HASH} hex digits)")).into());
        }
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let projects = inner.recent_traces(&set, LOOKUP_PROJECTS)?;
            let found = stories::lookup_commit(&hash, &projects, |repo| inner.remote_of(repo));
            Ok(found.ok_or_else(|| Error::NotFound(format!("no commit {hash} in your projects")))?)
        })
        .await
    }
}
