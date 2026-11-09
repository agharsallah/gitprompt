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

use std::collections::BTreeSet;

use gitprompt_core::Json;
use gitprompt_core::spec::Spec;
use gitprompt_store::{EventRow, PromptView};

/// Keys covered by the spec's data: mapped, model-call values, answers, rule paths, shown, ignored,
/// discarded; normalized (`[0]` -> `[]`).
pub fn declared(_spec: &Spec) -> BTreeSet<String> {
    todo!()
}

/// Every raw payload key the derivations read when they run over these prompts (each with the events
/// of its turn: same `prompt_id`), their facts, and the sessions of the events (same `session_id`).
/// Normalized like [`declared`]. Read keys of nested objects are recorded with their full path
/// (`tool_input.command`), list items with `[]`.
pub fn derived_reads(_spec: &Spec, _prompts: &[PromptView], _events: &[EventRow]) -> BTreeSet<String> {
    todo!()
}

/// The keys of `payload` (flattened, normalized, sorted, distinct) neither declared nor read.
pub fn uncovered(_payload: &Json, _declared: &BTreeSet<String>, _reads: &BTreeSet<String>) -> Vec<String> {
    todo!()
}
