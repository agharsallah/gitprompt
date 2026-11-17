//! Resuming a session: how to pick it up again (what the dashboard copies and `gitprompt resume`
//! prints), and which session "the last one" or a name stands for.

use gitprompt_core::Error;
use gitprompt_core::spec::{Resume, Spec};
use gitprompt_insights::derivations;
use gitprompt_insights::find::{Kind, PER_KIND, Target, session_hits};
use gitprompt_store::PromptFilter;
use gitprompt_store::rows::EventRow;
use gitprompt_store::stats::{AggregateQuery, GroupBy};

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::{FindHit, FindQuery};

/// How to resume one session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionResume {
    /// The tool's name, e.g. `Claude Code`.
    pub tool: String,
    /// `None` when the tool's spec has no resume template (or the session lacks what it needs).
    pub resume: Option<Resume>,
}

/// The session a reference stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionPick {
    One(FindHit),
    /// Several sessions answer to the name, best first: the caller says which and asks for an id.
    Several(Vec<FindHit>),
    Nothing,
}

impl Inner {
    /// The events of session `sid` and how to resume it under `spec`: the folder is the session's
    /// first, else `prompt_cwd`. The one place a session's resume action is worked out.
    pub(super) fn session_with_resume(
        &self,
        sid: &str,
        spec: Option<&Spec>,
        prompt_cwd: Option<&str>,
    ) -> gitprompt_core::Result<(Vec<EventRow>, Option<Resume>)> {
        let events = self.store.events_for_session(sid)?;
        let resume = spec.and_then(|spec| derivations::resume(spec, sid, &events, prompt_cwd));
        Ok((events, resume))
    }

    /// The session `id` as a find hit (its name or label, when it was last active); `None` when
    /// no prompt belongs to it.
    fn session_hit(&self, id: &str) -> ApiResult<Option<FindHit>> {
        let filter = PromptFilter { session: vec![id.to_owned()], ..PromptFilter::default() };
        let query = AggregateQuery { filter, group_by: Some(GroupBy::Session), limit: 1, ..AggregateQuery::default() };
        let mut groups = self.store.aggregate(&query)?.groups;
        self.name_sessions(&mut groups)?;
        Ok(session_hits(&groups, "", 1).pop())
    }
}

impl Service {
    /// How to resume session `id`, worked out as the Sessions list and story do
    /// ([`Inner::present_sessions`]), so the terminal prints what the dashboard copies; `NotFound`
    /// when nothing of the session was recorded.
    pub async fn resume_session(&self, id: &str) -> ApiResult<SessionResume> {
        let id = id.to_owned();
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let home = inner.store.session_homes(std::slice::from_ref(&id))?.remove(&id);
            let home = home.ok_or_else(|| Error::NotFound("session not found".into()))?;
            let spec = home.source.as_deref().and_then(|source| set.get(source));
            let resume = spec.and_then(|spec| spec.resume_action(&id, home.cwd.as_deref()));
            let tool = spec.map(|spec| spec.name.clone()).or(home.source).unwrap_or_else(|| "this tool".into());
            Ok(SessionResume { tool, resume })
        })
        .await
    }

    /// The session `reference` stands for: its id, else the name (or label, or title) it is called
    /// by, found the way the palette finds sessions; a name that exactly one session answers to
    /// wins over longer ones that merely contain it. No reference: the most recent session.
    pub async fn find_session(&self, reference: Option<&str>) -> ApiResult<SessionPick> {
        let Some(reference) = reference.map(str::trim).filter(|r| !r.is_empty()) else {
            return Ok(pick(self.sessions_matching("").await?, ""));
        };
        let id = reference.to_owned();
        if let Some(hit) = self
            .run(move |inner| {
                inner.indexed()?;
                inner.session_hit(&id)
            })
            .await?
        {
            return Ok(SessionPick::One(hit));
        }
        Ok(pick(self.sessions_matching(reference).await?, reference))
    }

    async fn sessions_matching(&self, q: &str) -> ApiResult<Vec<FindHit>> {
        Ok(self.find(FindQuery { q: q.to_owned(), kinds: vec![Kind::Sessions], limit: PER_KIND }).await?.sessions)
    }
}

/// One session out of the hits for `reference`; with no reference, the first is the latest.
fn pick(mut hits: Vec<FindHit>, reference: &str) -> SessionPick {
    if reference.is_empty() {
        return hits.into_iter().next().map_or(SessionPick::Nothing, SessionPick::One);
    }
    let called = |hit: &FindHit| hit.title.as_deref().is_some_and(|title| title.eq_ignore_ascii_case(reference));
    if hits.iter().filter(|hit| called(hit)).count() == 1 {
        hits.retain(called);
    }
    match hits.len() {
        0 => SessionPick::Nothing,
        1 => SessionPick::One(hits.remove(0)),
        _ => SessionPick::Several(hits),
    }
}

/// The id of a session hit.
pub fn session_id(hit: &FindHit) -> Option<&str> {
    match &hit.target {
        Target::Session { id } => Some(id),
        _ => None,
    }
}
