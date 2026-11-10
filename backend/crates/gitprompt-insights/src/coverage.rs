//! Field coverage (ADR-0021): every key a connector's payloads carry is used, or declared unused
//! with a reason. A tool update that adds a field then fails the conformance tests until someone
//! decides what it is for, instead of the field being stored and never shown.
//!
//! A payload key (a dotted path, list items as `[]`, `[0]` normalized to `[]`) is covered when it is
//!
//! * **mapped**: a candidate path of the spec's `map`, a `usage` value path, an `answer` path, or an
//!   origin or surface rule's `path`
//! * **derived**: read by the derivations (turn and session summaries, origin, surface, facts) when
//!   they run over the payloads ([`derived_reads`])
//! * **shown**: in the spec's `shown` (the dashboard reads it from the raw payload)
//! * **ignored**: in the spec's `ignored` or `discard` (never stored), with the reason
//!
//! A declared key covers what is under it. A key read as a whole covers what is under it, but a key
//! the derivations read *into* (`token_usage` -> `token_usage.input_tokens`) covers only what was
//! read.

use std::collections::{BTreeSet, HashMap};

use gitprompt_core::Json;
use gitprompt_core::json::flatten;
use gitprompt_core::spec::Spec;
use gitprompt_store::{EventRow, PromptView};

use crate::derivations::{derive_recorded, session_recorded};
use crate::raw::Reads;

/// Keys covered by the spec's data: mapped, model-call values, answers, rule paths, shown, ignored,
/// discarded; normalized (`[0]` -> `[]`).
pub fn declared(spec: &Spec) -> BTreeSet<String> {
    let mapped = spec.map.values().flatten();
    let rules = spec.origin.iter().map(|r| &r.when).chain(spec.surface.iter().map(|r| &r.when)).filter_map(|when| when.path.as_ref());
    mapped
        .chain(spec.usage.values.values())
        .chain(rules)
        .chain(spec.answer.values())
        .chain(spec.shown.keys())
        .chain(spec.ignored.keys())
        .chain(spec.discard.keys())
        .map(|path| normal(path))
        .collect()
}

/// `workspace_roots[0]` and `workspace_roots[]` name the same key.
fn normal(path: &str) -> String {
    path.replace("[0]", "[]")
}

/// `path` is `key` or lies under it.
fn under(path: &str, key: &str) -> bool {
    path.strip_prefix(key).is_some_and(|rest| rest.is_empty() || rest.starts_with('.') || rest.starts_with('['))
}

/// Every raw payload key the derivations read when they run over these prompts (each with the events
/// of its turn: same `prompt_id`), their facts, and the sessions of the events (same `session_id`).
/// Normalized like [`declared`]. Read keys of nested objects are recorded with their full path
/// (`tool_input.command`), list items with `[]`.
pub fn derived_reads(spec: &Spec, prompts: &[PromptView], events: &[EventRow]) -> BTreeSet<String> {
    let reads = Reads::default();
    for prompt in prompts {
        let turn: Vec<EventRow> = match prompt.row.fields.get("prompt_id").and_then(Json::as_str).filter(|pid| !pid.is_empty()) {
            Some(pid) => events.iter().filter(|e| e.prompt_id.as_deref() == Some(pid)).cloned().collect(),
            None => Vec::new(),
        };
        // Facts read only the derived values, never a payload: deriving covers them.
        derive_recorded(prompt, &turn, Some(spec), &reads);
    }
    let mut sessions: HashMap<&str, Vec<EventRow>> = HashMap::new();
    for e in events {
        if let Some(session) = e.session_id.as_deref().filter(|s| !s.is_empty()) {
            sessions.entry(session).or_default().push(e.clone());
        }
    }
    for session in sessions.values() {
        session_recorded(session, &reads);
    }
    reads.into_paths().iter().map(|path| normal(path)).collect()
}

/// The keys of `payload` (flattened, normalized, sorted, distinct) neither declared nor read.
pub fn uncovered(payload: &Json, declared: &BTreeSet<String>, reads: &BTreeSet<String>) -> Vec<String> {
    let covered = |path: &str| {
        declared.iter().any(|key| under(path, key))
            || reads.contains(path)
            || reads.iter().any(|key| under(path, key) && !reads.iter().any(|r| r != key && under(r, key)))
    };
    let keys: BTreeSet<String> =
        flatten(payload).into_iter().map(|(path, _)| normal(&path)).filter(|path| !path.is_empty() && !covered(path)).collect();
    keys.into_iter().collect()
}
