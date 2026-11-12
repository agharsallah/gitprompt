//! A prompt's facts: the few values every page shows about a turn, computed in one place (ADR-0021).
//! The prompt list and detail, stories, search results and exports all read [`Facts`], never pick
//! their own values out of `fields` and `summary`. Facts are built from an index row (as
//! [`crate::indexer::entry`] projects it) and the source spec, so they need no `if source == ...`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::JsonMap;
use gitprompt_core::spec::{Spec, SurfaceRef, UsageValue};
use gitprompt_store::Outcome;
use gitprompt_store::index::IndexRow;
use gitprompt_store::timeline::IndexedPrompt;

use crate::Specs;
use crate::derivations::TurnSummary;
use crate::text::{round, text_of};

/// What each fact means (listed on the Fields page).
pub const DOCS: &[(&str, &str)] = &[
    ("facts.connector", "the tool that sent the prompt: spec id, name, short name for badges, and who answers"),
    ("facts.surface", "where the prompt was typed: id, name and icon of the app or client (spec `surface` rules); null when unknown"),
    (
        "facts.model / model_source",
        "the model that answered: of the main thread's model calls, the one of most calls (usage; never a background or subagent call's model), else the model its hooks reported (hook)",
    ),
    ("facts.models", "every model the turn used"),
    ("facts.tokens", "input, output, cache read, cache write, reasoning and total tokens; null when no model call reported any"),
    ("facts.cost_usd", "the turn's cost; null unless a model call reported one"),
    ("facts.api_requests", "model calls reported for the turn"),
    ("facts.outcome", "how the turn ended: done, failed, interrupted, superseded, running or open"),
    ("facts.permission_mode / tool_version", "as the tool reported them with the prompt"),
    ("facts.response_s / answered", "time until the turn ended, and whether the end carried the final answer"),
    ("facts.tool_calls / tool_failures / files_written / git_commits / subagents", "what the turn did"),
];

/// The tool that sent a prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Connector {
    /// Spec id.
    pub id: String,
    pub name: String,
    /// Badge label: the spec's `short_name`, else its name.
    pub label: String,
    /// Who answers, e.g. `Codex`; `""` when unknown.
    pub assistant: String,
}

/// Token counts of a turn. Reasoning tokens are part of the output tokens: `total` counts them once
/// (input + output + cache read + cache creation).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "TurnTokens")]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_creation: i64,
    pub reasoning: i64,
    pub total: i64,
}

/// Where the main model came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    /// The turn's model calls.
    Usage,
    /// The model the tool's hooks reported with the prompt.
    Hook,
}

/// What every page shows about a turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Facts {
    pub connector: Connector,
    /// `None` when the tool doesn't say where the prompt was typed.
    pub surface: Option<SurfaceRef>,
    /// The main model.
    pub model: Option<String>,
    /// Every model the turn used (given), else the main model alone.
    pub models: Vec<String>,
    /// `usage` with model calls, `hook` without; `None` without a model.
    pub model_source: Option<ModelSource>,
    /// `None` when no model call was reported, or the calls carried no token counts (a cancelled
    /// Codex call): unknown, never zeros.
    pub tokens: Option<Tokens>,
    /// `None` without model calls, or when no call reported a cost and the spec's `usage` maps no
    /// `cost_usd` (the tool can't report one); `0.0` for free calls of a tool that reports costs.
    /// Rounded to 4 decimals.
    pub cost_usd: Option<f64>,
    pub api_requests: i64,
    pub outcome: Option<Outcome>,
    pub permission_mode: Option<String>,
    pub tool_version: Option<String>,
    pub response_s: Option<f64>,
    /// The turn's end carried the final answer.
    pub answered: bool,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub files_written: i64,
    pub git_commits: i64,
    pub subagents: i64,
}

/// The model that answered: the main thread's model ([`TurnSummary::main_model`]), else the
/// non-empty `model` field the tool reported with the prompt. Background calls (titles, web search
/// helpers, summaries, compaction) and subagents often run another, cheaper model and can outnumber
/// the main thread's calls, so they never decide it; a turn that has only those shows no model.
pub fn main_model(summary: &TurnSummary, fields: &JsonMap) -> Option<String> {
    summary.main_model.clone().or_else(|| fields.get("model").and_then(text_of).filter(|m| !m.is_empty()))
}

/// The connector of a source: from its spec, else named by the source id everywhere (assistant `""`).
pub fn connector(source: &str, spec: Option<&Spec>) -> Connector {
    match spec {
        Some(spec) => {
            Connector { id: spec.id.clone(), name: spec.name.clone(), label: spec.label().to_owned(), assistant: spec.assistant.clone() }
        }
        None => Connector { id: source.into(), name: source.into(), label: source.into(), assistant: String::new() },
    }
}

/// The facts of one prompt from its index row. `models`: every model of the turn, when known.
pub fn facts(row: &IndexRow, spec: Option<&Spec>, outcome: Option<Outcome>, models: Option<Vec<String>>) -> Facts {
    let called = row.usage.api_requests != 0;
    let model = row.model.clone();
    Facts {
        connector: connector(&row.source, spec),
        surface: spec.and_then(|spec| spec.surface_info(row.surface.as_deref())),
        models: models.unwrap_or_else(|| model.iter().cloned().collect()),
        model_source: model.as_ref().map(|_| if called { ModelSource::Usage } else { ModelSource::Hook }),
        model,
        tokens: tokens(row),
        cost_usd: cost(row, spec),
        api_requests: row.usage.api_requests,
        outcome,
        permission_mode: row.permission_mode.clone(),
        tool_version: row.tool_version.clone(),
        response_s: row.response_s,
        answered: row.answered,
        tool_calls: row.tool_calls,
        tool_failures: row.tool_failures,
        files_written: row.files_written,
        git_commits: row.git_commits,
        subagents: row.subagents,
    }
}

/// Tokens of the turn's model calls; `None` without calls or when they carried no counts.
fn tokens(row: &IndexRow) -> Option<Tokens> {
    let u = &row.usage;
    if u.api_requests == 0 {
        return None;
    }
    // Reasoning tokens are part of the output tokens: the total counts them once.
    let total = u.input_tokens + u.output_tokens + u.cache_read_tokens + u.cache_creation_tokens;
    (total != 0).then_some(Tokens {
        input: u.input_tokens,
        output: u.output_tokens,
        cache_read: u.cache_read_tokens,
        cache_creation: u.cache_creation_tokens,
        reasoning: row.reasoning_tokens,
        total,
    })
}

/// The turn's cost, when a call reported one or the tool reports costs at all.
fn cost(row: &IndexRow, spec: Option<&Spec>) -> Option<f64> {
    let cost = row.usage.cost_usd;
    let reports = spec.is_some_and(|spec| spec.usage.values.contains_key(&UsageValue::CostUsd));
    (row.usage.api_requests != 0 && (cost != 0.0 || reports)).then(|| round(cost, 4))
}

/// An indexed prompt (timeline row with its outcome) and its facts: what stories, search and exports
/// read. `facts` is `None` until [`attach`]ed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactRow {
    #[serde(flatten)]
    pub prompt: IndexedPrompt,
    pub facts: Option<Facts>,
}

/// Index rows with their facts (each with its source's spec and its outcome), in the same order.
pub fn attach(rows: Vec<IndexedPrompt>, specs: &Specs) -> Vec<FactRow> {
    rows.into_iter()
        .map(|prompt| {
            let spec = specs.get(&prompt.row.source).map(AsRef::as_ref);
            let facts = facts(&prompt.row, spec, Some(prompt.outcome), None);
            FactRow { prompt, facts: Some(facts) }
        })
        .collect()
}
