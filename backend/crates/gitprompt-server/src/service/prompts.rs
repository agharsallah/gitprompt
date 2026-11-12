//! The prompt list and detail, and the user's edits to one prompt (notes, redaction).

use std::collections::HashMap;
use std::path::Path;

use gitprompt_core::git::CommitsAfter;
use gitprompt_core::spec::Spec;
use gitprompt_core::{Error, Json, secrets};
use gitprompt_insights::derivations::{self, Derived, SessionSummary};
use gitprompt_insights::indexer::prompt_facts;
use gitprompt_store::PromptView;
use gitprompt_store::rows::EventRow;

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::{Annotation, Injected, PromptDetail, PromptOut, PromptQuery, Updated};

impl Service {
    /// Newest first, filtered on the prompt index.
    pub async fn prompts(&self, query: PromptQuery) -> ApiResult<Vec<PromptOut>> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let rows = store.search(&query.filter, query.limit)?;
            let turns: Vec<String> = rows.iter().filter_map(turn_id).collect();
            let events = by_turn(store.events_for_prompts(&turns)?);
            let outcomes = store.outcomes(&rows.iter().map(|r| r.row.id).collect::<Vec<_>>())?;
            Ok(rows
                .iter()
                .map(|row| {
                    let spec = set.get(&row.row.source).map(AsRef::as_ref);
                    let turn = turn_id(row).and_then(|pid| events.get(&pid)).map_or(&[][..], Vec::as_slice);
                    prompt_out(&derivations::derive(row, turn, spec), spec, outcomes.get(&row.row.id).copied())
                })
                .collect())
        })
        .await
    }

    pub async fn prompt(&self, id: i64) -> ApiResult<PromptDetail> {
        self.run(move |inner| {
            let (set, _) = inner.indexed()?;
            let store = &inner.store;
            let row = store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let events = match turn_id(&row) {
                Some(pid) => store.events_for_prompts(&[pid])?,
                None => Vec::new(),
            };
            let spec = set.get(&row.row.source).map(AsRef::as_ref);
            let outcome = store.outcomes(&[id])?.get(&id).copied();
            let prompt = prompt_out(&derivations::derive(&row, &events, spec), spec, outcome);
            let session = match row.row.fields.get("session_id").and_then(Json::as_str) {
                Some(sid) if !sid.is_empty() => derivations::session(&store.events_for_session(sid)?),
                _ => SessionSummary::default(),
            };
            let commits = commits_after(inner, &row);
            Ok(PromptDetail { prompt, raw: row.row.raw.clone(), session, commits, events })
        })
        .await
    }

    pub async fn annotate(&self, id: i64, annotation: Annotation) -> ApiResult<Annotation> {
        if !(-1..=1).contains(&annotation.rating) {
            return Err(ApiError::Unprocessable(format!("rating must be -1, 0 or 1, got {}", annotation.rating)));
        }
        self.run(move |inner| {
            inner.store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            inner.store.annotate(id, &annotation.clone().into())?;
            Ok(annotation)
        })
        .await
    }

    /// Hide detected secrets in the prompt, its raw payload, its events and the index.
    pub async fn redact(&self, id: i64) -> ApiResult<Updated> {
        self.run(move |inner| {
            let (_, indexer) = inner.indexed()?;
            inner.store.get(id)?.ok_or_else(|| Error::NotFound("prompt not found".into()))?;
            let updated = inner.store.rewrite(id, &secrets::redact)?;
            indexer.reindex(&inner.store, &[id])?;
            Ok(Updated { updated })
        })
        .await
    }
}

/// Events grouped by the turn (prompt id) they belong to; events of no turn are left out.
fn by_turn(events: Vec<EventRow>) -> HashMap<String, Vec<EventRow>> {
    let mut grouped: HashMap<String, Vec<EventRow>> = HashMap::new();
    for event in events {
        let Some(pid) = event.prompt_id.clone() else { continue };
        grouped.entry(pid).or_default().push(event);
    }
    grouped
}

/// A prompt as the list shows it, from its derivations and facts.
fn prompt_out(derived: &Derived, spec: Option<&Spec>, outcome: Option<gitprompt_store::Outcome>) -> PromptOut {
    let row = &derived.prompt.row;
    PromptOut {
        id: row.id,
        source: row.source.clone(),
        received_at: row.received_at.clone(),
        prompt: row.prompt.clone(),
        origin: derived.origin(),
        typed_text: derived.classified.typed_text.clone(),
        injected: derived.classified.injected.iter().map(|i| Injected { kind: i.kind.clone(), chars: i.chars as u64 }).collect(),
        fields: row.fields.clone(),
        context: row.context.clone(),
        annotation: derived.prompt.annotation.clone().into(),
        stats: derived.stats.clone(),
        summary: derived.summary.clone(),
        facts: prompt_facts(derived, spec, outcome),
    }
}

/// The id a prompt's turn events carry.
fn turn_id(view: &PromptView) -> Option<String> {
    view.row.fields.get("prompt_id").and_then(Json::as_str).filter(|pid| !pid.is_empty()).map(str::to_owned)
}

/// Commits made in the prompt's checkout until the next prompt of the session (or now); empty
/// (`{}` on the wire) outside a repository.
fn commits_after(inner: &Inner, row: &PromptView) -> CommitsAfter {
    let context = &row.row.context;
    let text = |key: &str| context.get(key).and_then(Json::as_str).filter(|v| !v.is_empty());
    let Some(root) = text("git_root") else { return CommitsAfter::default() };
    let checkout = text("git_worktree_root").unwrap_or(root); // a worktree has its own HEAD
    let end = row.next_head.as_deref().unwrap_or("HEAD");
    inner.options.git.commits(Path::new(checkout), text("git_head"), end, &row.row.received_at)
}
