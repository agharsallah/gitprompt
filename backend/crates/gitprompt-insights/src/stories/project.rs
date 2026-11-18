//! How a project got built, prompt by prompt ([`super::project_story`]).

use gitprompt_store::Outcome;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Diffstat, PhaseReason};
use crate::facts::Facts;
use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use gitprompt_core::git::Commit;
use gitprompt_core::spec::ToolKind;

use super::{
    BRANCH_RUN, EXCERPT_CHARS, FactRow, GAP_S, ProjectRef, RETRY_WINDOW, attribute_commits, clip, diffstat_of, is_user, non_empty, phases,
    project_ref, retries,
};
use crate::text::round;

/// A prompt of a project story.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "StoryEntry")]
pub struct ProjectEntry {
    pub id: i64,
    pub received_at: String,
    pub session: Option<String>,
    pub origin: String,
    pub branch: Option<String>,
    /// The typed text clipped to [`EXCERPT_CHARS`].
    pub text: String,
    pub clipped: bool,
    pub chars: i64,
    pub outcome: Outcome,
    pub response_s: Option<f64>,
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub tool_failures: i64,
    pub tools: BTreeMap<ToolKind, i64>,
    pub files_written: i64,
    pub git_commits: i64,
    pub commits: Vec<Commit>,
    pub retry_of: Option<i64>,
    pub facts: Option<Facts>,
}

/// What a phase (or the whole project) amounted to, and how its prompts were written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "StoryMeasures")]
pub struct Measures {
    pub prompts: u64,
    /// Distinct sessions.
    pub sessions: u64,
    /// Rounded to 4 decimals.
    pub cost_usd: f64,
    pub tool_calls: i64,
    pub files_written: i64,
    pub outcomes: IndexMap<Outcome, u64>,
    pub diffstat: Option<Diffstat>,
    /// Entries that reword an earlier prompt.
    pub retries: u64,
    /// Median typed characters, rounded half to even (Python `round`); 0 without prompts.
    pub median_chars: i64,
    /// Prompts with an @mention.
    pub with_mention: u64,
    /// Prompts with a code block.
    pub with_code: u64,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "Phase")]
pub struct ProjectPhase {
    /// 1-based.
    pub index: usize,
    pub reason: PhaseReason,
    pub gap_s: Option<i64>,
    /// Distinct branches, in order of appearance.
    pub branches: Vec<String>,
    #[serde(flatten)]
    pub measures: Measures,
    pub items: Vec<ProjectEntry>,
}

/// How a project got built, prompt by prompt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ProjectStory {
    /// From the typed prompts (else every row); with no row naming a repo: `repo` as id source, name
    /// and repo, `git: false`.
    #[serde(flatten)]
    pub project: ProjectRef,
    /// Prompts of other origins in the project: counted, never shown.
    pub system: i64,
    pub totals: Measures,
    pub phases: Vec<ProjectPhase>,
}

/// The prompts you typed in a project (rows of every origin, oldest first; the others are left out
/// but carry a prompt's work on, [`attribute_commits`]), split into [`phases`] with the default
/// rules, each prompt with the commits made in its work and its retry.
pub fn project_story(
    repo: &str,
    rows: &[FactRow],
    tool_kinds: &BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    commits: &[Commit],
    system: i64,
) -> ProjectStory {
    story(repo, rows, tool_kinds, commits, system)
}

pub(super) fn story(
    repo: &str,
    rows: &[FactRow],
    tool_kinds: &BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    commits: &[Commit],
    system: i64,
) -> ProjectStory {
    let user: Vec<FactRow> = rows.iter().filter(|p| is_user(p)).cloned().collect();
    let mut owned = attribute_commits(rows, commits);
    let retry = retries(&user, RETRY_WINDOW);
    let entries: Vec<ProjectEntry> = user
        .iter()
        .map(|p| {
            let id = p.prompt.row.id;
            entry(p, tool_kinds.get(&id).cloned().unwrap_or_default(), owned.remove(&id).unwrap_or_default(), retry.get(&id).copied())
        })
        .collect();
    let phases = phases(&user, GAP_S, BRANCH_RUN)
        .into_iter()
        .enumerate()
        .map(|(n, phase)| {
            let items = entries.get(phase.range.clone()).unwrap_or_default().to_vec();
            let mut branches: Vec<String> = Vec::new();
            for branch in items.iter().filter_map(|e| non_empty(e.branch.as_deref())) {
                if !branches.iter().any(|b| b == branch) {
                    branches.push(branch.to_owned());
                }
            }
            ProjectPhase {
                index: n + 1,
                reason: phase.reason,
                gap_s: phase.gap_s,
                branches,
                measures: measures(&items, user.get(phase.range).unwrap_or_default()),
                items,
            }
        })
        .collect();
    ProjectStory { project: project_ref(repo, rows, &user), system, totals: measures(&entries, &user), phases }
}

fn entry(p: &FactRow, tools: BTreeMap<ToolKind, i64>, commits: Vec<Commit>, retry_of: Option<i64>) -> ProjectEntry {
    let row = &p.prompt.row;
    let (text, clipped) = clip(row.typed_text.as_deref(), EXCERPT_CHARS);
    ProjectEntry {
        id: row.id,
        received_at: row.received_at.clone(),
        session: row.session.clone(),
        origin: row.origin.clone(),
        branch: row.branch.clone(),
        text,
        clipped,
        chars: row.chars,
        outcome: p.prompt.outcome,
        response_s: row.response_s,
        cost_usd: row.usage.cost_usd,
        tool_calls: row.tool_calls,
        tool_failures: row.tool_failures,
        tools,
        files_written: row.files_written,
        git_commits: row.git_commits,
        commits,
        retry_of,
        facts: p.facts.clone(),
    }
}

/// What some entries amounted to; `prompts` are their rows (for how they were written).
fn measures(entries: &[ProjectEntry], prompts: &[FactRow]) -> Measures {
    let mut outcomes = IndexMap::new();
    for e in entries {
        *outcomes.entry(e.outcome).or_default() += 1;
    }
    let sessions: BTreeSet<&str> = entries.iter().filter_map(|e| non_empty(e.session.as_deref())).collect();
    // Counts are far below u64::MAX: the casts are lossless.
    let count = |n: usize| n as u64;
    Measures {
        prompts: count(entries.len()),
        sessions: count(sessions.len()),
        cost_usd: round(entries.iter().map(|e| e.cost_usd).sum(), 4),
        tool_calls: entries.iter().map(|e| e.tool_calls).sum(),
        files_written: entries.iter().map(|e| e.files_written).sum(),
        outcomes,
        diffstat: diffstat_of(entries.iter().flat_map(|e| &e.commits)),
        retries: count(entries.iter().filter(|e| e.retry_of.is_some()).count()),
        median_chars: median(entries.iter().map(|e| e.chars).collect()),
        with_mention: count(prompts.iter().filter(|p| p.prompt.row.mentions > 0).count()),
        with_code: count(prompts.iter().filter(|p| p.prompt.row.code_blocks > 0).count()),
        first_at: entries.first().map(|e| e.received_at.clone()),
        last_at: entries.last().map(|e| e.received_at.clone()),
    }
}

/// Python's `round(statistics.median(values))`: the middle value, or the mean of the two middle
/// ones rounded half to even; 0 without values.
fn median(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    let n = values.len();
    #[expect(clippy::indexing_slicing, reason = "n > 0 in both arms: n / 2 < n, and n / 2 >= 1 when n is even")]
    match n {
        0 => 0,
        _ if n % 2 == 1 => values[n / 2],
        // Character counts: exactly representable as f64, and so is their half.
        #[expect(clippy::cast_possible_truncation, reason = "the rounded mean of two i64 counts fits an i64")]
        _ => f64::midpoint(values[n / 2 - 1] as f64, values[n / 2] as f64).round_ties_even() as i64,
    }
}
