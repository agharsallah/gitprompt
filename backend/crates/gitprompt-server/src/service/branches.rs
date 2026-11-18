//! Which project a branch is in, for `gitprompt why --branch`: the dashboard always names the
//! project, the terminal names only the branch (and, when it must, the project).

use gitprompt_insights::stories::{USER_ORIGINS, project_id};
use gitprompt_store::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, Bucket, GroupBy, Order};

use super::find::names_repo;
use super::{Inner, Service};
use crate::error::ApiResult;

/// Repos read, the most recently active first.
const REPOS: usize = 1000;

impl Inner {
    /// The repo where you last typed a prompt on `branch`, among those `project` names when given.
    fn branch_repo(&self, branch: &str, project: Option<&str>) -> ApiResult<Option<String>> {
        let query = AggregateQuery {
            filter: PromptFilter { origin: USER_ORIGINS.to_vec(), ..PromptFilter::default() },
            group_by: Some(GroupBy::Repo),
            split_by: Some(GroupBy::Branch),
            order: Some(Order::LastAt),
            limit: REPOS,
        };
        let groups = self.store.aggregate(&query)?.groups;
        let latest = groups
            .iter()
            .filter(|group| project.is_none_or(|project| names_repo(project, group)))
            .filter_map(|group| Some((on_branch(group, branch)?, group.key.as_deref()?)))
            .max_by(|a, b| a.0.cmp(b.0));
        Ok(latest.map(|(_, repo)| repo.to_owned()))
    }
}

/// When you last typed on `branch` in this repo group (ISO-8601 UTC, so it sorts as text); `None`
/// when you never did.
fn on_branch<'g>(group: &'g Bucket, branch: &str) -> Option<&'g str> {
    let part = group.parts.as_ref()?.iter().find(|part| part.key.as_deref() == Some(branch))?;
    part.last_at.as_deref()
}

impl Service {
    /// The project id of the repo where you last typed a prompt on `branch`, among the repos
    /// `project` names when given (its path, folder name or `owner/name`, as `gitprompt find`
    /// reads it); `None` when you typed none on that branch there.
    pub async fn branch_project(&self, branch: &str, project: Option<&str>) -> ApiResult<Option<String>> {
        let (branch, project) = (branch.to_owned(), project.map(str::to_owned));
        self.run(move |inner| {
            inner.indexed()?;
            Ok(inner.branch_repo(&branch, project.as_deref())?.map(|repo| project_id(&repo)))
        })
        .await
    }
}
