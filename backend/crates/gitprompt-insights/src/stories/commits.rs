//! Which prompt each commit followed, one rule read both ways: forward, a prompt's commits
//! ([`attribute_commits`], in the stories); backward, a commit's prompt ([`commit_traces`], the
//! Commits tab, `why`). Both ask [`Owners::owner`], so the two directions cannot disagree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::git::Commit;
use gitprompt_store::Outcome;

use super::{COMMIT_GRACE_S, EXCERPT_CHARS, FactRow, clip, ended, sent};
use crate::time::Instant;

/// How sure a commit's trace is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TraceHow {
    /// Made while its prompt's turn ran, or at most [`COMMIT_GRACE_S`] after it ended.
    Traced,
    /// Made later, with no prompt sent since: the rule gives it to that prompt, but it may as well
    /// have been made by hand.
    Likely,
    /// No prompt was sent before it (or its time can't be read).
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

/// A commit and the prompt behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CommitTrace {
    pub commit: Commit,
    /// `None` when made by hand.
    pub prompt: Option<TracePrompt>,
    /// The prompt's session.
    pub session: Option<String>,
    pub how: TraceHow,
}

/// One commit per trace, in the commits' order, each with the prompt behind it.
///
/// The rule: a commit belongs to the latest prompt (by `received_at`) sent at or before its time,
/// unless a turn that ran `git commit` (`git_commits > 0`) was still going, or had ended at most
/// [`COMMIT_GRACE_S`] earlier, when it was made: then to the latest such turn (that keeps parallel
/// sessions apart). It is [`TraceHow::Traced`] when made during its prompt's turn or within the
/// grace after it, [`TraceHow::Likely`] when made later (nothing else claims it, but nothing shows
/// the agent made it either), and [`TraceHow::ByHand`] when no prompt came before it or its time
/// can't be read. [`attribute_commits`] applies the same rule from the prompts' side: a commit
/// traced (or likely) to a prompt is exactly one of that prompt's commits there.
pub fn commit_traces(prompts: &[FactRow], commits: &[Commit]) -> Vec<CommitTrace> {
    let owners = Owners::new(prompts);
    commits
        .iter()
        .map(|commit| match owners.owner(commit).and_then(|owner| Some((prompts.get(owner.prompt)?, owner.how))) {
            Some((prompt, how)) => {
                CommitTrace { commit: commit.clone(), prompt: Some(trace_prompt(prompt)), session: prompt.prompt.row.session.clone(), how }
            }
            None => CommitTrace { commit: commit.clone(), prompt: None, session: None, how: TraceHow::ByHand },
        })
        .collect()
}

/// prompt id -> the commits made after it, in the commits' order: the rule of [`commit_traces`],
/// read from the prompts' side. Commits made by hand belong to none.
pub fn attribute_commits(prompts: &[FactRow], commits: &[Commit]) -> BTreeMap<i64, Vec<Commit>> {
    let owners = Owners::new(prompts);
    let mut out: BTreeMap<i64, Vec<Commit>> = BTreeMap::new();
    for commit in commits {
        if let Some(prompt) = owners.owner(commit).and_then(|owner| prompts.get(owner.prompt)) {
            out.entry(prompt.prompt.row.id).or_default().push(commit.clone());
        }
    }
    out
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

/// A prompt that can own commits: its time can be read.
struct Turn {
    /// Index into the prompts given.
    prompt: usize,
    sent: Instant,
    ended: Instant,
    /// The turn ran `git commit`.
    committed: bool,
}

impl Turn {
    /// The end of its turn plus [`COMMIT_GRACE_S`]: a commit up to then is the turn's own.
    fn grace_end(&self) -> Instant {
        self.ended.plus_seconds(COMMIT_GRACE_S as f64)
    }
}

/// The prompt a commit belongs to, and how sure that is.
#[derive(Clone, Copy, Debug)]
struct Owner {
    prompt: usize,
    how: TraceHow,
}

/// The prompts that can own commits, by time sent (stable: of two sent at once, the later given
/// is the later one).
struct Owners(Vec<Turn>);

impl Owners {
    fn new(prompts: &[FactRow]) -> Self {
        let mut turns: Vec<Turn> = prompts
            .iter()
            .enumerate()
            .filter_map(|(i, p)| Some(Turn { prompt: i, sent: sent(p)?, ended: ended(p)?, committed: p.prompt.row.git_commits > 0 }))
            .collect();
        turns.sort_by_key(|turn| turn.sent.at);
        Owners(turns)
    }

    /// The rule of [`commit_traces`]; `None`: made by hand.
    fn owner(&self, commit: &Commit) -> Option<Owner> {
        let at = Instant::parse(&commit.time)?.at;
        let before = self.0.get(..self.0.partition_point(|turn| turn.sent.at <= at)).unwrap_or_default();
        let committing = before.iter().rev().find(|turn| turn.committed && at <= turn.grace_end().at);
        let turn = committing.or_else(|| before.last())?;
        let how = if at <= turn.grace_end().at { TraceHow::Traced } else { TraceHow::Likely };
        Some(Owner { prompt: turn.prompt, how })
    }
}
