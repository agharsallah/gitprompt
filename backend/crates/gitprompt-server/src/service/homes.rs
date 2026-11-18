//! What sessions listed or told carry from where they ran: how to pick them up again (their tool's
//! spec and the folder they started in) and when they last did anything. One read for any number
//! of sessions ([`gitprompt_store::timeline::Home`]).

use std::collections::BTreeMap;

use gitprompt_core::spec::{Resume, SpecSet};
use gitprompt_store::stats::Bucket;

use super::Inner;
use crate::error::ApiResult;

/// A session's resume action and last activity; what a group or a story takes from its home.
#[derive(Debug, Default)]
pub(super) struct Presence {
    pub resume: Option<Resume>,
    pub last_event_at: Option<String>,
    /// Its latest prompt, and the owner/name of its remote.
    pub latest_prompt: Option<i64>,
    pub project: Option<String>,
}

impl Inner {
    /// Fill `resume` and `last_event_at` of session groups (`key` is the session id).
    pub(super) fn present_sessions(&self, set: &SpecSet, groups: &mut [Bucket]) -> ApiResult<()> {
        let sessions: Vec<String> = groups.iter().filter_map(|group| group.key.clone()).collect();
        let mut present = self.presence(set, &sessions)?;
        for group in groups {
            let Some(found) = group.key.as_deref().and_then(|key| present.remove(key)) else { continue };
            group.resume = found.resume;
            group.last_event_at = found.last_event_at;
        }
        Ok(())
    }

    /// The presence of `session`; empty when it has no prompt.
    pub(super) fn presence_of_session(&self, set: &SpecSet, session: &str) -> ApiResult<Presence> {
        Ok(self.presence(set, &[session.to_owned()])?.remove(session).unwrap_or_default())
    }

    pub(super) fn presence(&self, set: &SpecSet, sessions: &[String]) -> ApiResult<BTreeMap<String, Presence>> {
        if sessions.is_empty() {
            return Ok(BTreeMap::new());
        }
        let homes = self.store.session_homes(sessions)?;
        Ok(homes
            .into_iter()
            .map(|(session, home)| {
                let spec = home.source.as_deref().and_then(|source| set.get(source));
                let resume = spec.and_then(|spec| spec.resume_action(&session, home.cwd.as_deref()));
                (
                    session,
                    Presence { resume, last_event_at: home.last_event_at, latest_prompt: home.latest_prompt, project: home.repo_name },
                )
            })
            .collect())
    }
}
