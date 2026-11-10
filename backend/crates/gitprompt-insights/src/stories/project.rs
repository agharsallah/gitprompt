//! How a project got built, prompt by prompt ([`super::project_story`]).

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use gitprompt_core::git::Commit;
use gitprompt_core::spec::ToolKind;

use super::{
    BRANCH_RUN, EXCERPT_CHARS, FactRow, GAP_S, Measures, ProjectEntry, ProjectPhase, ProjectRef, ProjectStory, RETRY_WINDOW,
    attribute_commits, clip, diffstat_of, is_user, non_empty, phases, project_id, project_of, retries,
};
use crate::text::round;

pub(super) fn story(
    repo: &str,
    rows: &[FactRow],
    tool_kinds: &BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    commits: &[Commit],
    system: i64,
) -> ProjectStory {
    let user: Vec<FactRow> = rows.iter().filter(|p| is_user(p)).cloned().collect();
    let mut owned = attribute_commits(&user, commits);
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
            let items = entries[phase.range.clone()].to_vec();
            let mut branches: Vec<String> = Vec::new();
            for branch in items.iter().filter_map(|e| non_empty(&e.branch)) {
                if !branches.iter().any(|b| b == branch) {
                    branches.push(branch.to_owned());
                }
            }
            ProjectPhase {
                index: n + 1,
                reason: phase.reason,
                gap_s: phase.gap_s,
                branches,
                measures: measures(&items, &user[phase.range]),
                items,
            }
        })
        .collect();
    let source = if user.is_empty() { rows } else { &user };
    let project =
        project_of(source).unwrap_or_else(|| ProjectRef { id: project_id(repo), repo: repo.into(), name: repo.into(), git: false });
    ProjectStory { project, system, totals: measures(&entries, &user), phases }
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
    let sessions: BTreeSet<&str> = entries.iter().filter_map(|e| non_empty(&e.session)).collect();
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
    match n {
        0 => 0,
        _ if n % 2 == 1 => values[n / 2],
        // Character counts: exactly representable as f64, and so is their half.
        _ => ((values[n / 2 - 1] as f64 + values[n / 2] as f64) / 2.0).round_ties_even() as i64,
    }
}
