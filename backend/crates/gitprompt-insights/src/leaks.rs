//! Possible leaks in one turn: secrets and personal data in what you sent, in the answer, and in
//! what the tools read and returned, plus the sensitive files the agent read. Pure, and the clear
//! value never leaves this module: a [`Finding`] holds the value masked, and two keys derived
//! from it ([`Finding::id`], [`Finding::value_key`]) so a state set on a finding still applies after
//! a reindex, and a value you dismissed stays dismissed wherever it shows up again.
//!
//! Detection is [`gitprompt_core::secrets::find_leaks`] and [`gitprompt_core::personal`]; this
//! module decides where to look and how many to keep.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use gitprompt_core::Json;
use gitprompt_core::leak::{Found, Mask, Scope, masked};
use gitprompt_core::personal::{find_personal, sensitive_file};
use gitprompt_core::secrets::find_leaks;
use gitprompt_store::EventRow;

use crate::derivations::Derived;

/// Characters of one text scanned at most (a long tool output is read from its start).
pub const SCAN_CHARS: usize = 256 * 1024;
/// Findings kept per turn at most (a dump of addresses is one problem, not a thousand).
pub const PER_TURN: usize = 20;

/// Where in a turn a finding sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "LeakPlace")]
pub enum Place {
    /// What you sent.
    Prompt,
    /// The agent's answer.
    Answer,
    /// What a tool was asked (a command, a file path).
    ToolInput,
    /// What a tool returned (a file's contents, a command's output).
    ToolOutput,
}

impl Place {
    pub const fn as_str(self) -> &'static str {
        match self {
            Place::Prompt => "prompt",
            Place::Answer => "answer",
            Place::ToolInput => "tool_input",
            Place::ToolOutput => "tool_output",
        }
    }

    pub fn parse(name: &str) -> Option<Place> {
        [Place::Prompt, Place::Answer, Place::ToolInput, Place::ToolOutput].into_iter().find(|p| p.as_str() == name)
    }
}

/// One possible leak of one turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Stable: the kind, the turn, the place and the value.
    pub id: String,
    /// The kind and the value only: the same value wherever it appears.
    pub value_key: String,
    pub kind: String,
    pub masked: String,
    pub place: Place,
}

/// The possible leaks of the turn of `derived`, whose events are `events`; at most [`PER_TURN`],
/// each id once, in the order of the places above.
pub fn findings(derived: &Derived, events: &[EventRow]) -> Vec<Finding> {
    let turn = derived.prompt.row.id;
    let mut out: Vec<Finding> = Vec::new();
    for (place, found) in found(derived, events) {
        let finding = finding(turn, place, found);
        if out.len() >= PER_TURN {
            break;
        }
        if !out.iter().any(|f| f.id == finding.id) {
            out.push(finding);
        }
    }
    out
}

/// Every look-alike of the turn with where it sits, in the order of [`Place`].
fn found<'a>(derived: &'a Derived, events: &'a [EventRow]) -> Vec<(Place, Found<'a>)> {
    let mut out: Vec<(Place, Found<'a>)> = Vec::new();
    let prompt = derived.prompt.row.prompt.as_deref().map(scan).unwrap_or_default();
    out.extend(prompt.into_iter().map(|f| (Place::Prompt, f)));
    let answer = derived.summary.response.iter().flat_map(strings).flat_map(scan);
    out.extend(answer.map(|f| (Place::Answer, f)));
    let files =
        derived.summary.files_read.iter().filter_map(|path| {
            Some(Found { kind: sensitive_file(path)?, value: file_name(path), mask: Mask::Plain, scope: Scope::Anywhere })
        });
    out.extend(files.map(|f| (Place::ToolInput, f)));
    for (place, key) in [(Place::ToolInput, "tool_input"), (Place::ToolOutput, "tool_response")] {
        let tool = events.iter().filter_map(|e| e.raw.get(key)).flat_map(strings).flat_map(scan);
        out.extend(tool.filter(|f| place != Place::ToolOutput || f.scope == Scope::Anywhere).map(|f| (place, f)));
    }
    out
}

/// Secrets, then personal data, in the start of `text`.
fn scan(text: &str) -> Vec<Found<'_>> {
    let text = match text.char_indices().nth(SCAN_CHARS) {
        Some((end, _)) => text.get(..end).unwrap_or(text),
        None => text,
    };
    let mut found = find_leaks(text);
    found.extend(find_personal(text));
    found
}

/// Every string in a JSON value.
fn strings(value: &Json) -> Vec<&str> {
    match value {
        Json::String(text) => vec![text.as_str()],
        Json::Array(items) => items.iter().flat_map(strings).collect(),
        Json::Object(map) => map.values().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}

/// The last part of a path: where a secret is, without the folders (and the user name) above it.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn finding(turn: i64, place: Place, found: Found<'_>) -> Finding {
    let value_key = key(&[found.kind, &normal(found.value)]);
    let id = key(&[found.kind, &turn.to_string(), place.as_str(), &value_key]);
    Finding { id, value_key, kind: found.kind.to_owned(), masked: masked(found.mask, found.value), place }
}

/// The value as compared: no spaces, any case (`DE44 5001` is `de445001`).
fn normal(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

/// A stable digest of the parts: the first 16 hex digits of their SHA-256, so stored states keep
/// matching across versions and machines.
fn key(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().iter().take(8).fold(String::with_capacity(16), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
