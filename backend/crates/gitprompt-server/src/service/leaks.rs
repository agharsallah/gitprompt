//! Possible leaks: the board of a range (`GET /api/leaks`), and what you decide about one finding
//! (`PUT /api/leaks/{id}`): hide its value, mark it not sensitive, or open it again. The store
//! reads and writes; insights decides and finds the value again to hide it.

use std::collections::BTreeSet;

use gitprompt_core::spec::SpecSet;
use gitprompt_core::{Error, Json};
use gitprompt_insights::derivations::derive;
use gitprompt_insights::leaks::{LeakDecision, Leaks, allow_keys, board, redactor};
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::leaks::{LeakState, StoredLeak};

use super::prompts::turn_id;
use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::{LeakIn, LeakSaved};
use crate::query::iso;

impl Service {
    pub async fn leaks(&self, filter: PromptFilter) -> ApiResult<Leaks> {
        self.run(move |inner| {
            inner.indexed()?;
            inner.leak_board(&filter)
        })
        .await
    }

    /// Hide a finding's value, mark it not sensitive, or (no state) open it again.
    pub async fn set_leak(&self, id: String, body: LeakIn) -> ApiResult<LeakSaved> {
        self.run(move |inner| match body.state {
            Some(state) => inner.decide(id, state),
            None => inner.reopen(id),
        })
        .await
    }
}

impl Inner {
    fn decide(&self, id: String, state: LeakDecision) -> ApiResult<LeakSaved> {
        let (set, indexer) = self.indexed()?;
        let leak = self.store.leak(&id)?.ok_or_else(|| Error::NotFound(format!("no finding {id}")))?;
        let updated = if state == LeakDecision::Hidden { self.hide(&set, indexer.private_terms(), &leak)? } else { 0 };
        self.store.set_leak_state(&decision(&leak, state, &iso(self.now())))?;
        Ok(LeakSaved { id, state: Some(state), updated })
    }

    fn reopen(&self, id: String) -> ApiResult<LeakSaved> {
        if !self.store.clear_leak_state(&id)? {
            return Err(Error::NotFound(format!("no decision about finding {id}")).into());
        }
        Ok(LeakSaved { id, state: None, updated: 0 })
    }

    /// The possible leaks the filter keeps, with your decisions and the allow-list applied.
    pub(super) fn leak_board(&self, filter: &PromptFilter) -> ApiResult<Leaks> {
        let found = self.store.leaks(filter)?;
        let prompts = self.store.count(filter)?;
        Ok(board(found, &self.store.leak_states()?, &self.leak_allow()?, prompts))
    }

    /// Value keys never shown: your git email and the `leak_allow` preference.
    fn leak_allow(&self) -> ApiResult<BTreeSet<String>> {
        let mut values = self.preferences.current()?.leak_allow;
        values.extend(self.options.git.user_email());
        Ok(allow_keys(&values))
    }

    /// Replace the finding's value in its prompt and turn, then index the prompt again (the
    /// finding is gone from the index after); `terms` are the private terms it was indexed with.
    /// Rows rewritten.
    fn hide(&self, set: &SpecSet, terms: &[String], leak: &StoredLeak) -> ApiResult<u64> {
        let view = self.store.get(leak.prompt)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
        let events = match turn_id(&view) {
            Some(turn) => self.store.events_for_prompts(&[turn])?,
            None => Vec::new(),
        };
        let derived = derive(&view, &events, set.get(&view.row.source).map(AsRef::as_ref));
        let redactor = redactor(&derived, &events, &leak.finding, terms)
            .ok_or_else(|| Error::Conflict("the value is no longer in the stored text".into()))?;
        self.rewrite_turn(leak.prompt, &|json: &Json| redactor.apply(json))
    }
}

fn decision(leak: &StoredLeak, state: LeakDecision, at: &str) -> LeakState {
    LeakState {
        finding: leak.finding.clone(),
        value_key: leak.value_key.clone(),
        kind: leak.kind.clone(),
        masked: leak.masked.clone(),
        place: leak.place.clone(),
        prompt: leak.prompt,
        state: state.as_str().to_owned(),
        at: at.to_owned(),
    }
}
