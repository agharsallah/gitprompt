//! A typed prompt's versions ([`super::prompt_lineage`]).

use gitprompt_store::Outcome;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ProjectRef;
use super::{FactRow, MAX_VERSIONS, VERSION_CHARS, clip, is_user, non_empty, project_of};
use crate::lineage::{Change, Op};
use crate::lineage::{LENGTH_RATIO, Shingles, THRESHOLD, change, word_diff};

/// Candidates whose raw sizes differ more than `LENGTH_RATIO × SIZE_SLACK` can't pass
/// [`LENGTH_RATIO`] once normalized (normalizing removes a little): skipped before any shingling.
/// Where a version was typed, relative to the prompt asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    /// The prompt asked about.
    #[serde(rename = "self")]
    Itself,
    /// The same session.
    Session,
    /// Another session of the same repo.
    Project,
    Elsewhere,
}

/// One version of a prompt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Version {
    pub id: i64,
    pub received_at: String,
    pub session: Option<String>,
    pub project: Option<ProjectRef>,
    pub origin: String,
    /// Clipped to [`VERSION_CHARS`].
    pub text: String,
    pub clipped: bool,
    /// To the prompt asked about (1.0 for itself).
    pub similarity: f64,
    /// Same text hash as the prompt asked about (both non-empty).
    pub same_text: bool,
    pub place: Place,
    pub outcome: Outcome,
    pub response_s: Option<f64>,
    pub cost_usd: f64,
    pub tool_failures: i64,
    /// From the previous version's typed text; `None` for the first.
    pub diff: Option<Vec<(Op, String)>>,
    /// From the previous version's outcome; `None` for the first.
    pub change: Option<Change>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "Lineage")]
pub struct PromptLineage {
    pub id: i64,
    /// [`crate::lineage::THRESHOLD`]
    pub threshold: f64,
    /// Oldest first (by `received_at`, then id).
    pub versions: Vec<Version>,
}

/// The versions of a typed prompt: itself and the user prompts among `candidates` whose typed text is
/// similar enough ([`crate::lineage::THRESHOLD`]), at most [`MAX_VERSIONS`] (the most similar
/// kept). Candidates whose `chars` differ from the target's beyond `LENGTH_RATIO × 0.8` are skipped
/// before comparing. A prompt of a system origin has no versions.
pub fn prompt_lineage(target: &FactRow, candidates: &[FactRow]) -> PromptLineage {
    lineage(target, candidates)
}

const SIZE_SLACK: f64 = 0.8;

pub(super) fn lineage(target: &FactRow, candidates: &[FactRow]) -> PromptLineage {
    let id = target.prompt.row.id;
    if !is_user(target) {
        return PromptLineage { id, threshold: THRESHOLD, versions: Vec::new() };
    }
    let mine = Shingles::new(target.prompt.row.typed_text.as_deref().unwrap_or_default());
    // Character counts are far below 2^53: exact as f64.
    let size = target.prompt.row.chars.max(1) as f64;
    let mut scored: Vec<(f64, &FactRow)> = candidates
        .iter()
        .filter(|c| c.prompt.row.id != id && is_user(c))
        .filter(|c| {
            let chars = c.prompt.row.chars as f64;
            chars.min(size) / chars.max(size).max(1.0) >= LENGTH_RATIO * SIZE_SLACK
        })
        .filter_map(|c| {
            let score = mine.similarity(&Shingles::new(c.prompt.row.typed_text.as_deref().unwrap_or_default()));
            (score >= THRESHOLD).then_some((score, c))
        })
        .collect();
    // Most similar first (stable: equal scores keep the candidates' order), the closest kept.
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate(MAX_VERSIONS - 1);
    scored.push((1.0, target));
    scored.sort_by(|a, b| {
        let (a, b) = (&a.1.prompt.row, &b.1.prompt.row);
        (a.received_at.as_str(), a.id).cmp(&(b.received_at.as_str(), b.id))
    });
    let mut versions: Vec<Version> = Vec::with_capacity(scored.len());
    let mut previous: Option<&FactRow> = None;
    for (similarity, p) in scored {
        let row = &p.prompt.row;
        let (text, clipped) = clip(row.typed_text.as_deref(), VERSION_CHARS);
        versions.push(Version {
            id: row.id,
            received_at: row.received_at.clone(),
            session: row.session.clone(),
            project: project_of(std::slice::from_ref(p)),
            origin: row.origin.clone(),
            text,
            clipped,
            similarity,
            same_text: non_empty(row.text_hash.as_deref()).is_some() && row.text_hash == target.prompt.row.text_hash,
            place: place(p, target),
            outcome: p.prompt.outcome,
            response_s: row.response_s,
            cost_usd: row.usage.cost_usd,
            tool_failures: row.tool_failures,
            diff: previous.map(|before| word_diff(before.prompt.row.typed_text.as_deref(), row.typed_text.as_deref())),
            change: previous.and_then(|before| change(Some(before.prompt.outcome), Some(p.prompt.outcome))),
        });
        previous = Some(p);
    }
    PromptLineage { id, threshold: THRESHOLD, versions }
}

fn place(p: &FactRow, target: &FactRow) -> Place {
    let (row, target) = (&p.prompt.row, &target.prompt.row);
    if row.id == target.id {
        Place::Itself
    } else if non_empty(row.session.as_deref()).is_some() && row.session == target.session {
        Place::Session
    } else if row.repo == target.repo {
        Place::Project
    } else {
        Place::Elsewhere
    }
}
