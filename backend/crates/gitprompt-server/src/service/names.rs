//! The names you give things (a session now): stored durably, shown next to the default label
//! ([`gitprompt_insights::label`]) wherever sessions are listed or told.

use gitprompt_core::Error;
use gitprompt_insights::label::session_label;
use gitprompt_insights::stories::USER_ORIGINS;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::Bucket;
use gitprompt_store::store::{NameKind, clean_name};

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::NameOut;

/// Typed prompts of a session read when it has no title: the first with words gives the label.
const LABEL_PROMPTS: usize = 5;

impl Inner {
    /// Fill `name` and `label` of session groups (`key` is the session id).
    pub(super) fn name_sessions(&self, groups: &mut [Bucket]) -> ApiResult<()> {
        let sessions: Vec<String> = groups.iter().filter_map(|group| group.key.clone()).collect();
        if sessions.is_empty() {
            return Ok(());
        }
        let mut names = self.store.names_of(NameKind::Session, &sessions)?;
        let openers = self.store.session_openers(&sessions, &USER_ORIGINS, LABEL_PROMPTS)?;
        for group in groups {
            let Some(key) = group.key.as_deref() else { continue };
            group.label = openers.get(key).and_then(|o| session_label(o.title.as_deref(), o.typed.iter().map(String::as_str)));
            group.name = names.remove(key);
        }
        Ok(())
    }

    /// What you called this session, if you did.
    pub(super) fn name_of_session(&self, session: &str) -> ApiResult<Option<String>> {
        Ok(self.store.names_of(NameKind::Session, &[session.to_owned()])?.remove(session))
    }
}

/// The kind of thing `kind` spells (`session`); anything else is not found.
fn kind_of(kind: &str) -> ApiResult<NameKind> {
    NameKind::parse(kind).ok_or_else(|| ApiError::NotFound(format!("you cannot name a {kind}")))
}

impl Service {
    /// Call `id` of `kind` by `name` (a session must exist); the name as stored.
    pub async fn set_name(&self, kind: &str, id: &str, name: &str) -> ApiResult<NameOut> {
        let kind = kind_of(kind)?;
        let name = clean_name(name).map_err(|e| ApiError::Unprocessable(e.to_string()))?;
        let id = id.to_owned();
        self.run(move |inner| {
            inner.indexed()?;
            let known = match kind {
                NameKind::Session => inner.store.count(&PromptFilter { session: vec![id.clone()], ..PromptFilter::default() })? > 0,
            };
            if !known {
                return Err(Error::NotFound(format!("{} not found", kind.as_str())).into());
            }
            Ok(NameOut { name: inner.store.set_name(kind, &id, &name)? })
        })
        .await
    }

    /// Forget the name of `id` of `kind`; nothing happens when it had none.
    pub async fn clear_name(&self, kind: &str, id: &str) -> ApiResult<()> {
        let kind = kind_of(kind)?;
        let id = id.to_owned();
        self.run(move |inner| inner.store.clear_name(kind, &id).map(drop).map_err(ApiError::from)).await
    }
}
