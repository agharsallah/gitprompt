//! Prompt stories (ADR-0016): ordered, explained views of what you typed, for one session or one
//! project, plus a prompt's versions. Built from timeline rows with their facts
//! ([`crate::facts::FactRow`]) and commits from `gitprompt_core::git::history`; no I/O here, so an
//! export can render the same story to another format.
//!
//! A story never shows system-originated text as typed: only [`USER_ORIGINS`] become chapters or
//! project prompts; everything else is a marker, labelled by its origin.
//!
//! Times are ISO-8601 with an offset or `Z`. A prompt's turn *ends* at `received_at + response_s`
//! (no `response_s`: at `received_at`).

mod commit_list;
mod commit_lookup;
mod commits;
mod ids;
mod owners;
mod project;
mod session;
mod versions;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::Commit;
use gitprompt_core::spec::Origin;

use crate::facts::FactRow;
use crate::lineage::Shingles;
use crate::time::Instant;
pub use commit_list::{
    CommitGroup, CommitGrouping, CommitQuery, ProjectCommits, ProjectTraces, TraceCounts, project_commits, project_traces,
};
pub use commit_lookup::{CommitLookup, FoundCommit, lookup_commit};
pub use commits::{CommitTrace, MIN_HASH, TraceAlternative, TraceHow, TracePrompt, attribute_commits, commit_traces, names_hash};
pub use project::{Measures, ProjectEntry, ProjectPhase, ProjectStory, project_story};
pub use session::{
    Chapter, Marker, MarkerKind, SessionItem, SessionStory, SessionTotals, StoryItem, TurnTools, group_markers, session_story,
};
pub use versions::{Place, PromptLineage, Version, prompt_lineage};

/// Origins whose text the user wrote.
pub const USER_ORIGINS: [Origin; 2] = [Origin::Typed, Origin::SlashCommand];
/// A pause this long (seconds, from the end of the previous turn) starts a new phase.
pub const GAP_S: f64 = 3.0 * 3600.0;
/// A branch starts a new phase once it holds for this many prompts in a row.
pub const BRANCH_RUN: usize = 2;
/// Earlier prompts of the session compared when looking for a retry.
pub const RETRY_WINDOW: usize = 30;
/// A commit this many seconds after a prompt's work ended was still made in it.
pub const COMMIT_GRACE_S: i64 = 120;
/// A commit made while no work ran is likely the work that ended last (committed by hand) when
/// that ended at most this many seconds earlier; later, it is made by hand. The same pause as
/// [`GAP_S`]: after it a project story starts a new phase, so what you commit then is new work.
pub const LIKELY_S: i64 = 3 * 3600;
/// Seconds of commits read after the last turn of a story ended.
pub const LOG_TAIL_S: i64 = 2 * 3600;
/// Seconds of commits a project's commit list reads before its first prompt: work you did by hand
/// just before you started prompting.
pub const LOG_LEAD_S: i64 = 2 * 3600;
/// Typed text shown in a session chapter.
pub const CHAPTER_CHARS: usize = 4000;
/// Text shown in a marker or a project story entry.
pub const EXCERPT_CHARS: usize = 280;
/// File paths listed per chapter.
pub const MAX_FILES: usize = 20;
/// The prompt itself and its closest versions.
pub const MAX_VERSIONS: usize = 20;
/// Typed text shown per version.
pub const VERSION_CHARS: usize = 2000;

// --- project ids -------------------------------------------------------------------------------

/// A stable, URL-safe key for a project (git root or folder): base64url of its UTF-8 bytes, no
/// padding. Reversible, so the dashboard can build it too.
pub fn project_id(repo: &str) -> String {
    ids::encode(repo.as_bytes())
}

/// The git root or folder a project id names; `None` if the key is not valid base64url (`-_`
/// alphabet, padding optional), decodes to invalid UTF-8, or decodes to an empty string.
pub fn project_repo(key: &str) -> Option<String> {
    String::from_utf8(ids::decode(key)?).ok().filter(|repo| !repo.is_empty())
}

/// A project as stories name it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "StoryProject")]
pub struct ProjectRef {
    /// [`project_id`] of `repo`.
    pub id: String,
    pub repo: String,
    /// The first `repo_name` (owner/name) of the rows, else the last segment of `repo`.
    pub name: String,
    /// Some row has a branch: it is a git repository.
    pub git: bool,
}

/// The project of the first row that has a repo; `None` when none does.
pub fn project_of(rows: &[FactRow]) -> Option<ProjectRef> {
    let repo = rows.iter().find_map(|r| non_empty(r.prompt.row.repo.as_deref()))?;
    let name = rows.iter().find_map(|r| non_empty(r.prompt.row.repo_name.as_deref())).map_or_else(|| folder_name(repo), str::to_owned);
    Some(ProjectRef {
        id: project_id(repo),
        repo: repo.to_owned(),
        name,
        git: rows.iter().any(|r| non_empty(r.prompt.row.branch.as_deref()).is_some()),
    })
}

/// The project of the typed prompts `user` (else of every row); with no row naming a repo: `repo`
/// as id source, name and repo, `git: false`.
fn project_ref(repo: &str, rows: &[FactRow], user: &[FactRow]) -> ProjectRef {
    let source = if user.is_empty() { rows } else { user };
    project_of(source).unwrap_or_else(|| ProjectRef { id: project_id(repo), repo: repo.into(), name: repo.into(), git: false })
}

/// The last segment of a path, a trailing `/` ignored.
pub fn folder_name(repo: &str) -> String {
    let trimmed = repo.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_owned()
}

/// The text when it is set and not empty.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}

// --- pure building blocks ----------------------------------------------------------------------

/// Whether a prompt's text is the user's ([`USER_ORIGINS`]).
pub fn is_user(row: &FactRow) -> bool {
    crate::origin::parse(&row.prompt.row.origin).is_some_and(|origin| USER_ORIGINS.contains(&origin))
}

/// When a prompt was sent; `None` when its time can't be read.
fn sent(row: &FactRow) -> Option<Instant> {
    Instant::parse(&row.prompt.row.received_at)
}

/// When a prompt's turn ended: `received_at + response_s` (no `response_s`: when it was sent).
fn ended(row: &FactRow) -> Option<Instant> {
    Some(sent(row)?.plus_seconds(row.prompt.row.response_s.unwrap_or(0.0)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum PhaseReason {
    /// The first phase.
    Start,
    /// A pause of `gap_s` or more.
    Gap,
    /// The branch changed and the new one held.
    Branch,
}

/// A run of consecutive prompts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase {
    pub reason: PhaseReason,
    /// The pause that started it, in whole seconds (rounded half to even); `Gap` phases only.
    pub gap_s: Option<i64>,
    /// The phase's prompts: indexes into the prompts given.
    pub range: Range<usize>,
}

/// Split prompts (oldest first) into phases: a new phase starts when the pause from the end of the
/// previous turn is `gap_s` or more (`Gap`), or when the branch differs from the phase's branch and
/// the next `branch_run` prompts (this one included; fewer left = no) all have that branch
/// (`Branch`). Prompts without a branch never change it; the phase's branch is the first one seen.
/// The gap rule wins when both hold.
pub fn phases(prompts: &[FactRow], gap_s: f64, branch_run: usize) -> Vec<Phase> {
    let mut out: Vec<Phase> = Vec::new();
    let mut branch: Option<&str> = None;
    for (i, prompt) in prompts.iter().enumerate() {
        let mine = branch_of(prompt);
        if let Some((reason, gap_s)) = phase_start(prompts, i, branch, gap_s, branch_run) {
            out.push(Phase { reason, gap_s, range: i..i });
            branch = mine.or(branch);
        }
        branch = branch.or(mine);
        if let Some(phase) = out.last_mut() {
            phase.range.end = i + 1;
        }
    }
    out
}

/// Why prompt `i` starts a new phase (and the pause that did, `Gap` only), given the branch of
/// the phase so far; `None` when it continues that phase.
fn phase_start(prompts: &[FactRow], i: usize, branch: Option<&str>, gap_s: f64, branch_run: usize) -> Option<(PhaseReason, Option<i64>)> {
    let Some(previous) = i.checked_sub(1).and_then(|j| prompts.get(j)) else {
        return Some((PhaseReason::Start, None));
    };
    let rest = prompts.get(i..).unwrap_or_default();
    if let Some(gap) = rest.first().and_then(|prompt| pause(previous, prompt)).filter(|gap| *gap >= gap_s) {
        #[expect(clippy::cast_possible_truncation, reason = "whole seconds of a gap far below i64::MAX: the cast is exact")]
        let whole = gap.round_ties_even() as i64;
        return Some((PhaseReason::Gap, Some(whole)));
    }
    switches_branch(rest, branch, branch_run).then_some((PhaseReason::Branch, None))
}

/// Seconds from the end of `previous`'s turn to when `prompt` was sent; `None` when a time can't
/// be read.
fn pause(previous: &FactRow, prompt: &FactRow) -> Option<f64> {
    Some(sent(prompt)?.at.duration_since(ended(previous)?.at).as_secs_f64())
}

/// Whether the first of `rest` leaves `branch` for another one that the first `branch_run` of
/// `rest` all have (fewer left: no). Without a known branch on either side, nothing switches.
fn switches_branch(rest: &[FactRow], branch: Option<&str>, branch_run: usize) -> bool {
    let (Some(current), Some(next)) = (branch, rest.first().and_then(branch_of)) else { return false };
    let run = rest.get(..branch_run).filter(|run| !run.is_empty());
    next != current && run.is_some_and(|run| run.iter().all(|prompt| branch_of(prompt) == Some(next)))
}

/// A prompt's branch, when it has one.
fn branch_of(prompt: &FactRow) -> Option<&str> {
    non_empty(prompt.prompt.row.branch.as_deref())
}

/// prompt id -> id of the latest earlier prompt of the same session (rows without a session share
/// one) that it rewords or repeats ([`crate::lineage::is_version`] of the typed texts), among the
/// `window` prompts of that session before it.
pub fn retries(prompts: &[FactRow], window: usize) -> BTreeMap<i64, i64> {
    let mut by_session: HashMap<Option<&str>, Vec<(i64, Shingles)>> = HashMap::new();
    let mut out = BTreeMap::new();
    for p in prompts {
        let row = &p.prompt.row;
        let earlier = by_session.entry(row.session.as_deref()).or_default();
        let mine = Shingles::new(row.typed_text.as_deref().unwrap_or_default());
        let recent = earlier.get(earlier.len().saturating_sub(window)..).unwrap_or_default();
        if let Some((id, _)) = recent.iter().rev().find(|(_, other)| mine.is_version(other)) {
            out.insert(row.id, *id);
        }
        earlier.push((row.id, mine));
    }
    out
}

/// Commits, files, insertions and deletions summed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Diffstat {
    pub commits: u64,
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

/// `None` without commits.
pub fn diffstat(commits: &[Commit]) -> Option<Diffstat> {
    diffstat_of(commits.iter())
}

fn diffstat_of<'c>(commits: impl Iterator<Item = &'c Commit>) -> Option<Diffstat> {
    let mut out = Diffstat::default();
    for c in commits {
        out.commits += 1;
        out.files += c.files;
        out.insertions += c.insertions;
        out.deletions += c.deletions;
    }
    (out.commits > 0).then_some(out)
}

/// `(since, until)` of the commits a story reads: the first prompt's `received_at` as given, and
/// [`LOG_TAIL_S`] after the latest turn end (ISO-8601 like Python's `isoformat`: seconds, fractional
/// digits only when non-zero, the offset of that prompt). `None` without prompts.
pub fn log_window(prompts: &[FactRow]) -> Option<(String, String)> {
    let first = prompts.first()?;
    let last = latest_end(prompts)?;
    Some((first.prompt.row.received_at.clone(), last.plus_seconds(LOG_TAIL_S as f64).isoformat()))
}

/// `(since, until)` of the commits a project's commit list reads: from [`LOG_LEAD_S`] before the
/// first prompt was sent up to `until` (the caller's "now"). It is open at the end, unlike
/// [`log_window`], so a commit made while a turn still runs, or long after the last one, is listed
/// (and traced as the rule says: by hand when far from any work). `None` without prompts, or when
/// the first one's time can't be read.
pub fn trace_window(prompts: &[FactRow], until: &str) -> Option<(String, String)> {
    let since = sent(prompts.first()?)?.plus_seconds(-(LOG_LEAD_S as f64));
    Some((since.isoformat(), until.to_owned()))
}

/// The latest turn end (by instant, in the offset that prompt was written in).
fn latest_end(prompts: &[FactRow]) -> Option<Instant> {
    prompts.iter().filter_map(ended).max_by_key(|end| end.at)
}

/// Text clipped for a story: `(text, clipped)` ([`crate::text::clip`]); no text is `""`.
fn clip(text: Option<&str>, chars: usize) -> (String, bool) {
    crate::text::clip(text.unwrap_or_default(), chars)
}
