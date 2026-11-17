//! Find: the best prompts, sessions and projects for one query. The store answers each kind with
//! reads that already exist (the prompt filter on the typed-text index, statistics grouped by
//! session and by repo); `gitprompt_insights::find` ranks them. Sessions carry their names and
//! default labels (ticket 09) first, so find matches what the dashboard calls them.

use gitprompt_core::Result;
use gitprompt_insights::find::{Kind, matches, project_hits, prompt_hits, session_hits};
use gitprompt_insights::stories::USER_ORIGINS;
use gitprompt_store::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, Bucket, GroupBy, Order};
use gitprompt_store::store::NameKind;

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::{FindQuery, FindResults};

/// Candidates read per kind before ranking: the most recent ones. Prompts are matched by the store
/// over all time first, so only a finished older prompt behind 200 newer matches is missed. Sessions
/// and projects are matched here, so one older than the pool is found only by a name you gave it
/// (`named_beyond`) or through one of its prompts.
const POOL: usize = 200;

impl Inner {
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
                let filter = PromptFilter {
                    q: Some(q.to_owned()).filter(|q| !q.trim().is_empty()),
                    origin: USER_ORIGINS.to_vec(),
                    ..PromptFilter::default()
                };
                found.prompts = prompt_hits(&inner.store.timeline(&filter, Some(POOL), true)?, q, limit);
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
