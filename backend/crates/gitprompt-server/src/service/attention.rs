//! `GET /api/attention`: the sessions to pick up (always the latest) and what is worth a look in
//! the range the filters keep. The daemon reads; [`gitprompt_insights::attention`] decides.

use gitprompt_core::spec::{Origin, SpecSet};
use gitprompt_insights::attention::{Attention, Inputs, RepeatedText, SessionState, attention};
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, Bucket, GroupBy, Order};

use super::{Inner, Service};
use crate::error::ApiResult;

/// Latest sessions read for Pick up: enough for a full list after the ones left too long ago.
const SESSIONS_READ: usize = 10;
/// Most repeated texts read for Worth a look.
const TEXTS_READ: usize = 20;

impl Service {
    pub async fn attention(&self, filter: PromptFilter) -> ApiResult<Attention> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let redone = Some(inner.redone_work(&set, &filter)?);
            let leaks = inner.leak_board(&filter)?.open;
            let inputs = Inputs { sessions: inner.recent_sessions(&set)?, repeated: inner.repeated_texts(filter)?, redone, leaks };
            Ok(attention(&inputs, inner.now()))
        })
        .await
    }
}

impl Inner {
    /// The latest sessions, whatever the range, with their names, homes and latest turn's end.
    fn recent_sessions(&self, set: &SpecSet) -> ApiResult<Vec<SessionState>> {
        let query = AggregateQuery {
            group_by: Some(GroupBy::Session),
            order: Some(Order::LastAt),
            limit: SESSIONS_READ,
            ..AggregateQuery::default()
        };
        let mut groups = self.store.aggregate(&query)?.groups;
        self.name_sessions(&mut groups)?;
        let ids: Vec<String> = groups.iter().filter_map(|g| g.key.clone()).collect();
        let mut present = self.presence(set, &ids)?;
        let latest: Vec<i64> = present.values().filter_map(|p| p.latest_prompt).collect();
        let outcomes = self.store.outcomes(&latest)?;
        Ok(groups
            .into_iter()
            .filter_map(|group| {
                let id = group.key?;
                let home = present.remove(&id).unwrap_or_default();
                let ended = home.latest_prompt.and_then(|prompt| outcomes.get(&prompt).copied());
                Some(SessionState {
                    id,
                    name: group.name,
                    label: group.label,
                    project: home.project,
                    last_event_at: home.last_event_at,
                    latest_prompt: home.latest_prompt,
                    ended,
                    resume: home.resume,
                })
            })
            .collect())
    }

    /// The typed texts sent more than once in the range, most sent first, each with its whole text.
    fn repeated_texts(&self, filter: PromptFilter) -> ApiResult<Vec<RepeatedText>> {
        let filter = PromptFilter { origin: vec![Origin::Typed], ..filter };
        let query = AggregateQuery {
            filter,
            group_by: Some(GroupBy::Text),
            order: Some(Order::Prompts),
            limit: TEXTS_READ,
            ..AggregateQuery::default()
        };
        let groups: Vec<Bucket> = self.store.aggregate(&query)?.groups.into_iter().filter(|g| g.prompts > 1).collect();
        let hashes: Vec<String> = groups.iter().filter_map(|g| g.key.clone()).collect();
        let mut texts = self.store.typed_texts(&hashes)?;
        Ok(groups
            .into_iter()
            .filter_map(|g| Some(RepeatedText { text: texts.remove(g.key.as_deref()?)?, prompts: g.prompts, sessions: g.sessions }))
            .collect())
    }
}
