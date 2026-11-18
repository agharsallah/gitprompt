//! A project's commits, each with the prompt behind it ([`super::commit_traces`]), searched and
//! grouped for the Commits tab ([`project_commits`]).

use std::cmp::Reverse;
use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::Commit;

use super::{
    BRANCH_RUN, CommitTrace, FactRow, GAP_S, ProjectRef, TraceHow, commit_traces, is_user, names_hash, non_empty, phases, project_ref,
};
use crate::time::Instant;

/// What the commits are grouped by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum CommitGrouping {
    /// The project phase of the prompt behind the commit (as in the project story).
    #[default]
    Phase,
    /// The branch the prompt behind it was sent on: git does not record the branch a commit was
    /// made on.
    Branch,
    /// The commit's date, in the offset it was made in.
    Day,
}

/// What to list: commits matching every word of `q` (empty: all), grouped by `group`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitQuery {
    pub q: String,
    pub group: CommitGrouping,
}

/// How many commits followed a prompt, and how surely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TraceCounts {
    pub total: u64,
    pub traced: u64,
    pub likely: u64,
    pub by_hand: u64,
}

/// Commits sharing a phase, a branch or a day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CommitGroup {
    /// The phase's 1-based index, the branch or the day (`YYYY-MM-DD`); `None`: commits without
    /// one (made by hand, for a phase or a branch).
    pub key: Option<String>,
    pub count: u64,
    /// Newest first.
    pub commits: Vec<CommitTrace>,
}

/// A project's commits while you worked on it, each with the prompt behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ProjectCommits {
    #[serde(flatten)]
    pub project: ProjectRef,
    pub group: CommitGrouping,
    /// Every commit, whatever `q` matched.
    pub counts: TraceCounts,
    /// Commits matching `q`: those in `groups`.
    pub matched: u64,
    /// Newest first: a group comes where its newest commit does.
    pub groups: Vec<CommitGroup>,
}

/// A project's commits, each traced to the prompt you typed that it followed: the one read behind
/// its Commits tab, a commit found by its hash, and the commits find matches.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectTraces {
    pub project: ProjectRef,
    /// In the commits' order.
    pub traces: Vec<CommitTrace>,
}

/// The commits of a project (oldest first, as `git::history` lists them), each traced to the
/// prompt you typed that it followed (rows of every origin, oldest first: the others carry a
/// prompt's work on, [`commit_traces`]).
pub fn project_traces(repo: &str, rows: &[FactRow], commits: &[Commit]) -> ProjectTraces {
    traced(repo, rows, commits).1
}

/// [`project_traces`], and the prompts you typed.
fn traced(repo: &str, rows: &[FactRow], commits: &[Commit]) -> (Vec<FactRow>, ProjectTraces) {
    let user: Vec<FactRow> = rows.iter().filter(|p| is_user(p)).cloned().collect();
    let traces = ProjectTraces { project: project_ref(repo, rows, &user), traces: commit_traces(rows, commits) };
    (user, traces)
}

/// The commits of a project ([`project_traces`]), counted, then those matching `query.q` (each
/// word in the subject, a path or the hash, ignoring case) grouped, newest first by commit time.
pub fn project_commits(repo: &str, rows: &[FactRow], commits: &[Commit], query: &CommitQuery) -> ProjectCommits {
    let (user, ProjectTraces { project, traces }) = traced(repo, rows, commits);
    let words: Vec<String> = query.q.split_whitespace().map(str::to_lowercase).collect();
    let mut matched: Vec<CommitTrace> = traces.iter().rev().filter(|t| matches(&t.commit, &words)).cloned().collect();
    // Newest first even where git's order is not by date (a commit dated earlier than its parent).
    matched.sort_by_key(|t| Reverse(Instant::parse(&t.commit.time).map(|time| time.at)));
    let phase_of = phase_index(&user);
    let key = |trace: &CommitTrace| group_key(trace, query.group, &phase_of);
    ProjectCommits { project, group: query.group, counts: counts(&traces), matched: count(matched.len()), groups: group(matched, key) }
}

/// Whether every word is in the subject or a path, or names the hash ([`names_hash`]).
fn matches(commit: &Commit, words: &[String]) -> bool {
    let contains = |text: &str, word: &str| text.to_lowercase().contains(word);
    let names = |word: &str| names_hash(&commit.hash, word) || contains(&commit.subject, word);
    words.iter().all(|word| names(word) || commit.paths.iter().any(|path| contains(path, word)))
}

/// prompt id -> the 1-based index of its phase ([`phases`] with the default rules).
fn phase_index(user: &[FactRow]) -> HashMap<i64, usize> {
    let mut out = HashMap::new();
    for (n, phase) in phases(user, GAP_S, BRANCH_RUN).into_iter().enumerate() {
        for prompt in user.get(phase.range).unwrap_or_default() {
            out.insert(prompt.prompt.row.id, n + 1);
        }
    }
    out
}

fn group_key(trace: &CommitTrace, grouping: CommitGrouping, phase_of: &HashMap<i64, usize>) -> Option<String> {
    let prompt = trace.prompt.as_ref();
    match grouping {
        CommitGrouping::Phase => prompt.and_then(|p| phase_of.get(&p.id)).map(ToString::to_string),
        CommitGrouping::Branch => prompt.and_then(|p| non_empty(p.branch.as_deref())).map(str::to_owned),
        CommitGrouping::Day => trace.commit.time.get(..10).map(str::to_owned),
    }
}

/// Traces in groups by key, in the order each key first comes.
fn group(traces: Vec<CommitTrace>, key: impl Fn(&CommitTrace) -> Option<String>) -> Vec<CommitGroup> {
    let mut groups: IndexMap<Option<String>, Vec<CommitTrace>> = IndexMap::new();
    for trace in traces {
        groups.entry(key(&trace)).or_default().push(trace);
    }
    groups.into_iter().map(|(key, commits)| CommitGroup { key, count: count(commits.len()), commits }).collect()
}

fn counts(traces: &[CommitTrace]) -> TraceCounts {
    let of = |how: TraceHow| count(traces.iter().filter(|t| t.how == how).count());
    TraceCounts { total: count(traces.len()), traced: of(TraceHow::Traced), likely: of(TraceHow::Likely), by_hand: of(TraceHow::ByHand) }
}

/// Counts are far below `u64::MAX`: the cast is lossless.
fn count(n: usize) -> u64 {
    n as u64
}
