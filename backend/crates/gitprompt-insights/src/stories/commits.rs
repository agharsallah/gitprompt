//! Which prompt each commit followed, one rule read both ways: forward, a prompt's commits
//! ([`attribute_commits`], in the stories); backward, a commit's prompt ([`commit_traces`], the
//! Commits tab, `why`). Both ask [`Owners::owner`], so the two directions cannot disagree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::{Commit, commit_url};
use gitprompt_store::Outcome;

use super::owners::Owners;
use super::{EXCERPT_CHARS, FactRow, clip};

/// How sure a commit's trace is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceHow {
    /// Made while its prompt's work ran, and no other work could have made it.
    Traced,
    /// A guess: another prompt could own it (the trace names it), it was rewritten after its work,
    /// or it was made by hand after the work.
    Likely,
    /// Made long after any work, before every prompt, or at a time that can't be read.
    ByHand,
}

/// The prompt a commit followed, as a trace names it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct TracePrompt {
    pub id: i64,
    pub received_at: String,
    pub branch: Option<String>,
    /// The typed text clipped to [`EXCERPT_CHARS`].
    pub text: String,
    pub clipped: bool,
    pub outcome: Outcome,
}

/// The other prompt that could own a commit ([`commit_traces`] says when there is one).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct TraceAlternative {
    pub prompt: TracePrompt,
    pub session: Option<String>,
}

/// A commit and the prompt behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CommitTrace {
    pub commit: Commit,
    /// `None` when made by hand.
    pub prompt: Option<TracePrompt>,
    /// The prompt's session.
    pub session: Option<String>,
    pub how: TraceHow,
    /// The other prompt that could own it; only on a [`TraceHow::Likely`] trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub alternative: Option<TraceAlternative>,
    /// Its page on GitHub, GitLab or Bitbucket ([`CommitTrace::on_remote`]); left out without such
    /// a remote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
}

impl CommitTrace {
    /// The trace with its page on the forge of `remote` (the `origin` URL, `git::commit_url`):
    /// every front end links a commit the same way.
    #[must_use]
    pub fn on_remote(mut self, remote: Option<&str>) -> Self {
        self.url = remote.and_then(|remote| commit_url(remote, &self.commit.hash));
        self
    }
}

/// One commit per trace, in the commits' order, each with the prompt behind it. `rows` are the
/// prompts of every origin, oldest first; only the ones you typed own commits.
///
/// The rule. A prompt you typed starts a *work*: its turn, then the turns its session runs without
/// you (a subagent hands back, a background task ends) until you type there again, plus
/// [`super::COMMIT_GRACE_S`]. A commit is placed at its author time (an amend or a rebase keeps
/// it), among the works running then:
/// - exactly one of them ran `git commit`: [`TraceHow::Traced`] to it;
/// - two or more did, or none did and two or more ran: [`TraceHow::Likely`] to the later sent,
///   naming the other as `alternative` (two sessions in one repository);
/// - one ran without committing: `Traced` to it, unless the work that had ended last wrote files
///   and never committed them; then `Likely` to that one (you committed its changes by hand),
///   naming the running one;
/// - none ran: `Likely` to the work that ended last, if at most [`super::LIKELY_S`] earlier, else
///   (and before every prompt, or at a time that can't be read) [`TraceHow::ByHand`].
///
/// A commit rewritten after its work ended (committer time later: amended, rebased) is at most
/// `Likely`. A guess is never shown as certain. [`attribute_commits`] applies the same rule from
/// the prompts' side: a commit traced (or likely) to a prompt is one of that prompt's commits there.
pub fn commit_traces(rows: &[FactRow], commits: &[Commit]) -> Vec<CommitTrace> {
    let owners = Owners::new(rows);
    commits.iter().map(|commit| trace(rows, &owners, commit)).collect()
}

/// prompt id -> the commits made in its work, in the commits' order: the rule of
/// [`commit_traces`] (`rows` of every origin, as there), read from the prompts' side. Commits made
/// by hand belong to none; an alternative owner is not given the commit.
pub fn attribute_commits(rows: &[FactRow], commits: &[Commit]) -> BTreeMap<i64, Vec<Commit>> {
    let owners = Owners::new(rows);
    let mut out: BTreeMap<i64, Vec<Commit>> = BTreeMap::new();
    for commit in commits {
        if let Some(prompt) = owners.owner(commit).and_then(|owner| rows.get(owner.prompt)) {
            out.entry(prompt.prompt.row.id).or_default().push(commit.clone());
        }
    }
    out
}

fn trace(rows: &[FactRow], owners: &Owners, commit: &Commit) -> CommitTrace {
    let by_hand = CommitTrace { commit: commit.clone(), prompt: None, session: None, how: TraceHow::ByHand, alternative: None, url: None };
    let Some(owner) = owners.owner(commit) else { return by_hand };
    let Some(prompt) = rows.get(owner.prompt) else { return by_hand };
    let alternative = owner
        .alternative
        .and_then(|i| rows.get(i))
        .map(|other| TraceAlternative { prompt: trace_prompt(other), session: other.prompt.row.session.clone() });
    CommitTrace { prompt: Some(trace_prompt(prompt)), session: prompt.prompt.row.session.clone(), how: owner.how, alternative, ..by_hand }
}

/// The fewest characters of a hash that name a commit (git's own minimum).
pub const MIN_HASH: usize = 4;

/// Whether `word` names the commit `hash`, ignoring case: either is a prefix of the other, so a
/// full hash finds the short one git printed, and a prefix finds the commit.
pub fn names_hash(hash: &str, word: &str) -> bool {
    let (hash, word) = (hash.to_lowercase(), word.to_lowercase());
    !hash.is_empty() && !word.is_empty() && (hash.starts_with(&word) || word.starts_with(&hash))
}

fn trace_prompt(prompt: &FactRow) -> TracePrompt {
    let row = &prompt.prompt.row;
    let (text, clipped) = clip(row.typed_text.as_deref(), EXCERPT_CHARS);
    TracePrompt {
        id: row.id,
        received_at: row.received_at.clone(),
        branch: row.branch.clone(),
        text,
        clipped,
        outcome: prompt.prompt.outcome,
    }
}
