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

mod ids;
mod project;
mod session;
mod versions;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::Commit;
use gitprompt_core::spec::{Origin, Pattern, SurfaceRef, ToolKind};
use gitprompt_store::Outcome;
use gitprompt_store::timeline::TurnEvent;

use crate::facts::{Connector, FactRow, Facts};
use crate::lineage::{Change, Op, Shingles};
use crate::time::Instant;

/// Origins whose text the user wrote.
pub const USER_ORIGINS: [Origin; 2] = [Origin::Typed, Origin::SlashCommand];
/// A pause this long (seconds, from the end of the previous turn) starts a new phase.
pub const GAP_S: f64 = 3.0 * 3600.0;
/// A branch starts a new phase once it holds for this many prompts in a row.
pub const BRANCH_RUN: usize = 2;
/// Earlier prompts of the session compared when looking for a retry.
pub const RETRY_WINDOW: usize = 30;
/// A commit this many seconds after a committing turn ended still belongs to it.
pub const COMMIT_GRACE_S: i64 = 120;
/// Seconds of commits read after the last turn of a story ended.
pub const LOG_TAIL_S: i64 = 2 * 3600;
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

/// The last segment of a path, a trailing `/` ignored.
pub(crate) fn folder_name(repo: &str) -> String {
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
    for (i, p) in prompts.iter().enumerate() {
        let mine = non_empty(p.prompt.row.branch.as_deref());
        let start = if out.is_empty() {
            Some((PhaseReason::Start, None))
        } else {
            let previous = i.checked_sub(1).and_then(|j| prompts.get(j));
            let gap = match (sent(p), previous.and_then(ended)) {
                (Some(at), Some(previous)) => Some(at.at.duration_since(previous.at).as_secs_f64()),
                _ => None,
            };
            let run = prompts.get(i..prompts.len().min(i + branch_run)).unwrap_or_default();
            let holds = run.len() == branch_run && run.iter().all(|q| q.prompt.row.branch.as_deref() == mine);
            if let Some(gap) = gap.filter(|g| *g >= gap_s) {
                #[expect(clippy::cast_possible_truncation, reason = "whole seconds of a gap far below i64::MAX: the cast is exact")]
                let whole = gap.round_ties_even() as i64;
                Some((PhaseReason::Gap, Some(whole)))
            } else if branch.is_some() && mine.is_some_and(|b| Some(b) != branch) && branch_run > 0 && holds {
                Some((PhaseReason::Branch, None))
            } else {
                None
            }
        };
        if let Some((reason, gap_s)) = start {
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

/// prompt id -> the commits made after it, in the commits' order. A commit belongs to the latest
/// prompt (by `received_at`) sent at or before its time, unless a turn that ran `git commit`
/// (`git_commits > 0`) was still going, or had ended at most [`COMMIT_GRACE_S`] earlier, when it was
/// made: then to the latest such turn. That keeps parallel sessions apart. Commits before every
/// prompt, or whose time can't be parsed, belong to none.
pub fn attribute_commits(prompts: &[FactRow], commits: &[Commit]) -> BTreeMap<i64, Vec<Commit>> {
    // (id, sent, ended, committed) by time sent; prompts whose time can't be read can own nothing.
    let mut ordered: Vec<(i64, Instant, Instant, bool)> =
        prompts.iter().filter_map(|p| Some((p.prompt.row.id, sent(p)?, ended(p)?, p.prompt.row.git_commits > 0))).collect();
    ordered.sort_by_key(|(_, at, _, _)| at.at);
    let mut out: BTreeMap<i64, Vec<Commit>> = BTreeMap::new();
    for commit in commits {
        let Some(at) = Instant::parse(&commit.time) else { continue };
        let before = ordered.get(..ordered.partition_point(|(_, sent, _, _)| sent.at <= at.at)).unwrap_or_default();
        let committing =
            before.iter().rev().find(|(_, _, ended, committed)| *committed && at.at <= ended.plus_seconds(COMMIT_GRACE_S as f64).at);
        if let Some((owner, ..)) = committing.or_else(|| before.last()) {
            out.entry(*owner).or_default().push(commit.clone());
        }
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

/// The latest turn end (by instant, in the offset that prompt was written in).
fn latest_end(prompts: &[FactRow]) -> Option<Instant> {
    prompts.iter().filter_map(ended).max_by_key(|end| end.at)
}

// --- a session ---------------------------------------------------------------------------------

/// What a marker stands for: a prompt of a system origin, or something that happened in a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    /// A `PreCompact` event of a turn.
    Compaction,
    /// A call of an agent tool that has a brief.
    SubagentPrompt,
    TaskNotification,
    AgentMessage,
    Subagent,
    Programmatic,
    Unknown,
}

impl MarkerKind {
    /// The marker of a prompt of this origin (`Typed` and `SlashCommand` are never markers; they
    /// map to `Unknown`).
    pub fn of_origin(origin: Origin) -> Self {
        match origin {
            Origin::TaskNotification => MarkerKind::TaskNotification,
            Origin::AgentMessage => MarkerKind::AgentMessage,
            Origin::Subagent => MarkerKind::Subagent,
            Origin::Programmatic => MarkerKind::Programmatic,
            Origin::Typed | Origin::SlashCommand | Origin::Unknown => MarkerKind::Unknown,
        }
    }
}

/// System activity between chapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Marker {
    pub kind: MarkerKind,
    /// The prompt it belongs to.
    pub id: i64,
    pub received_at: String,
    /// A subagent prompt's task description.
    pub label: Option<String>,
    /// A subagent prompt's subagent type.
    pub detail: Option<String>,
    /// The text, clipped to [`EXCERPT_CHARS`] (cut, trailing whitespace removed, `…` added);
    /// `None` when empty.
    pub excerpt: Option<String>,
    pub clipped: bool,
}

/// A prompt you typed, with what its turn did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Chapter {
    pub id: i64,
    /// 1-based position among every prompt of the session (markers count).
    pub turn: usize,
    pub received_at: String,
    pub origin: String,
    /// The typed text clipped to [`CHAPTER_CHARS`] (`…` added when cut).
    pub text: String,
    pub clipped: bool,
    pub slash_command: Option<String>,
    pub branch: Option<String>,
    pub outcome: Outcome,
    pub response_s: Option<f64>,
    pub cost_usd: f64,
    /// Input + output tokens.
    pub tokens: i64,
    pub tool_calls: i64,
    pub tool_failures: i64,
    /// Tool calls per kind.
    pub tools: BTreeMap<ToolKind, i64>,
    pub files_written: i64,
    /// Sorted paths the turn's write tools touched, at most [`MAX_FILES`].
    pub files: Vec<String>,
    pub subagents: i64,
    pub git_commits: i64,
    pub commits: Vec<Commit>,
    pub diffstat: Option<Diffstat>,
    /// The earlier prompt of the session this one rewords.
    pub retry_of: Option<i64>,
    pub facts: Option<Facts>,
}

/// One item of a session, before markers are grouped.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionItem {
    Chapter(Box<Chapter>),
    Marker(Marker),
}

/// One item of a session story. Serializes with `kind`: `chapter` (the chapter's fields) or
/// `markers` (`items`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoryItem {
    Chapter(Box<Chapter>),
    Markers { items: Vec<Marker> },
}

/// Consecutive markers folded into one `Markers` item, so system activity between two chapters
/// reads as one collapsed group. Chapters stay as they are, in order.
pub fn group_markers(items: Vec<SessionItem>) -> Vec<StoryItem> {
    let mut out: Vec<StoryItem> = Vec::new();
    for item in items {
        match (item, out.last_mut()) {
            (SessionItem::Chapter(chapter), _) => out.push(StoryItem::Chapter(chapter)),
            (SessionItem::Marker(marker), Some(StoryItem::Markers { items })) => items.push(marker),
            (SessionItem::Marker(marker), _) => out.push(StoryItem::Markers { items: vec![marker] }),
        }
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SessionTotals {
    pub chapters: u64,
    /// Every marker, before grouping.
    pub markers: u64,
    /// Of every prompt of the session, rounded to 4 decimals.
    pub cost_usd: f64,
    /// Of every prompt of the session.
    pub tool_calls: i64,
    /// Distinct files listed in the chapters.
    pub files_written: u64,
    /// Chapters per outcome.
    pub outcomes: IndexMap<Outcome, u64>,
    pub retries: u64,
    /// Of the chapters' commits.
    pub diffstat: Option<Diffstat>,
}

/// One session as a narrative.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SessionStory {
    pub session: String,
    /// The first title of its prompts.
    pub title: Option<String>,
    /// The first prompt's source.
    pub source: Option<String>,
    /// The first prompt's connector (from its facts).
    pub connector: Option<Connector>,
    /// The first surface any prompt's facts name.
    pub surface: Option<SurfaceRef>,
    /// Prompts per main model (from their facts).
    pub models: IndexMap<String, u64>,
    pub project: Option<ProjectRef>,
    /// Distinct branches, in order of appearance.
    pub branches: Vec<String>,
    pub started_at: Option<String>,
    /// The latest turn end, ISO-8601.
    pub ended_at: Option<String>,
    pub totals: SessionTotals,
    pub items: Vec<StoryItem>,
}

/// What the session story reads about each turn besides its rows.
#[derive(Clone, Copy, Debug)]
pub struct TurnTools<'a> {
    /// Tool calls per kind of each prompt (by id).
    pub tool_kinds: &'a BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    /// The turns' finished tool calls and compactions, oldest first.
    pub events: &'a [TurnEvent],
    /// The source spec's `tool_kinds` and `tool_files` (empty without a spec).
    pub kinds: &'a IndexMap<String, ToolKind>,
    pub tool_files: &'a IndexMap<String, Pattern>,
}

/// One session (rows oldest first): each prompt you typed is a chapter (outcome, tools, files,
/// commits, cost, time, retry); prompts of other origins are markers (their typed text as excerpt),
/// and each turn's compactions and agent-tool calls with a brief become markers after its prompt;
/// markers are then folded into groups ([`group_markers`]). Commits are attributed among the
/// chapters only; retries are looked for among the chapters only.
pub fn session_story(session: &str, rows: &[FactRow], turns: TurnTools<'_>, commits: &[Commit]) -> SessionStory {
    session::story(session, rows, turns, commits)
}

// --- a project ---------------------------------------------------------------------------------

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

/// The prompts you typed in a project (rows oldest first; other origins are left out), split into
/// [`phases`] with the default rules, each prompt with the commits made after it and its retry.
pub fn project_story(
    repo: &str,
    rows: &[FactRow],
    tool_kinds: &BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    commits: &[Commit],
    system: i64,
) -> ProjectStory {
    project::story(repo, rows, tool_kinds, commits, system)
}

// --- a prompt's lineage ------------------------------------------------------------------------

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
    versions::lineage(target, candidates)
}

/// Text clipped for a story: `(text, clipped)` ([`crate::text::clip`]); no text is `""`.
fn clip(text: Option<&str>, chars: usize) -> (String, bool) {
    crate::text::clip(text.unwrap_or_default(), chars)
}
