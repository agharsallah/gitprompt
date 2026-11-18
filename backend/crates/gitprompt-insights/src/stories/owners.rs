//! The one rule behind commit traces ([`super::commit_traces`], [`super::attribute_commits`]):
//! whose work a commit belongs to, the other prompt that could own it, and how sure that is.

use std::collections::HashMap;

use jiff::Timestamp;

use gitprompt_core::git::Commit;

use super::{COMMIT_GRACE_S, FactRow, LIKELY_S, TraceHow, ended, is_user, sent};
use crate::time::Instant;

/// A prompt you typed and the work it started: its own turn, then the turns of its session that
/// carry it on without you (a subagent hands back, a background task ends) until you type there
/// again. Subagents and background tasks commit between those turns, so the work runs from the
/// prompt to the end of the last of them.
struct Work {
    /// Index into the rows given.
    prompt: usize,
    sent: Instant,
    /// When its last turn ended.
    ended: Instant,
    /// One of its turns ran `git commit`.
    committed: bool,
    /// One of its turns wrote a file.
    wrote: bool,
}

impl Work {
    /// The end of the work plus [`COMMIT_GRACE_S`]: a commit up to then is made while it ran.
    fn grace_end(&self) -> Timestamp {
        self.ended.plus_seconds(COMMIT_GRACE_S as f64).at
    }

    /// It wrote files and never committed them: they may be committed by hand after it.
    fn left_changes(&self) -> bool {
        self.wrote && !self.committed
    }

    /// How sure it is that this work made `commit`, the only one that could: certain, unless the
    /// commit was rewritten (amended, rebased) after the work ended and may hold more than it.
    fn sole_owner_of(&self, commit: &Commit) -> TraceHow {
        let rewritten = Instant::parse(&commit.time).is_some_and(|time| time.at > self.grace_end());
        if rewritten { TraceHow::Likely } else { TraceHow::Traced }
    }
}

/// The prompt a commit belongs to, as indexes into the rows given.
#[derive(Clone, Copy, Debug)]
pub(super) struct Owner {
    pub prompt: usize,
    /// The other prompt whose work could have made it.
    pub alternative: Option<usize>,
    pub how: TraceHow,
}

/// The works of the rows given, by time sent (stable: of two sent at once, the later given is the
/// later one).
pub(super) struct Owners {
    works: Vec<Work>,
    /// `reach[i]`: the index of the work, among `works[..=i]`, that ran the latest (grace included).
    reach: Vec<usize>,
}

/// A turn whose time can be read.
struct Turn {
    row: usize,
    sent: Instant,
    ended: Instant,
}

impl Owners {
    pub(super) fn new(rows: &[FactRow]) -> Self {
        let mut turns: Vec<Turn> =
            rows.iter().enumerate().filter_map(|(row, p)| Some(Turn { row, sent: sent(p)?, ended: ended(p)? })).collect();
        turns.sort_by_key(|turn| turn.sent.at);
        let works = works(rows, &turns);
        let mut reach: Vec<usize> = Vec::with_capacity(works.len());
        for (i, work) in works.iter().enumerate() {
            let latest = reach.last().copied().filter(|&j| works.get(j).is_some_and(|other| other.grace_end() > work.grace_end()));
            reach.push(latest.unwrap_or(i));
        }
        Owners { works, reach }
    }

    /// The rule of [`super::commit_traces`]; `None`: made by hand.
    pub(super) fn owner(&self, commit: &Commit) -> Option<Owner> {
        let made = made_at(commit)?;
        let before = self.works.partition_point(|work| work.sent.at <= made);
        let running = self.running(before, made);
        let committing: Vec<&Work> = running.iter().copied().filter(|work| work.committed).collect();
        match (committing.as_slice(), running.as_slice()) {
            // One running work ran `git commit`: it made the commit.
            ([work], _) => Some(Owner::of(work, None, work.sole_owner_of(commit))),
            // Two did, or two ran and neither did: the later sent, naming the other.
            ([.., other, work], _) | ([], [.., other, work]) => Some(Owner::of(work, Some(other), TraceHow::Likely)),
            // One ran without committing: you likely committed by hand what the work that had ended
            // last left uncommitted; when none did, the running work made the commit.
            (_, [work]) => match self.last_ended(before, made).filter(|done| done.left_changes()) {
                Some(done) => Some(Owner::of(done, Some(work), TraceHow::Likely)),
                None => Some(Owner::of(work, None, work.sole_owner_of(commit))),
            },
            // None ran: you committed the work that ended last by hand.
            (_, []) => self.last_ended(before, made).map(|work| Owner::of(work, None, TraceHow::Likely)),
        }
    }

    /// The works among the first `before` still running (grace included) at `made`, by time sent.
    /// Walks back only while some earlier work still reaches `made`.
    fn running(&self, before: usize, made: Timestamp) -> Vec<&Work> {
        let reaches = |i: usize| self.reach.get(i).and_then(|&j| self.works.get(j)).is_some_and(|work| work.grace_end() >= made);
        let mut running: Vec<&Work> = (0..before)
            .rev()
            .take_while(|&i| reaches(i))
            .filter_map(|i| self.works.get(i))
            .filter(|work| work.grace_end() >= made)
            .collect();
        running.reverse();
        running
    }

    /// The work, among the first `before`, that had ended (grace included) last at `made`, if it
    /// ended at most [`LIKELY_S`] earlier.
    fn last_ended(&self, before: usize, made: Timestamp) -> Option<&Work> {
        let done = self.works.get(..before)?.iter().filter(|work| work.grace_end() < made);
        done.max_by_key(|work| (work.ended.at, work.sent.at)).filter(|work| made <= work.ended.plus_seconds(LIKELY_S as f64).at)
    }
}

impl Owner {
    fn of(work: &Work, other: Option<&Work>, how: TraceHow) -> Self {
        Owner { prompt: work.prompt, alternative: other.map(|other| other.prompt), how }
    }
}

/// The works of `turns` (by time sent): each prompt you typed starts one, and the turns of other
/// origins carry on the latest one of their session (rows without a session share one).
fn works(rows: &[FactRow], turns: &[Turn]) -> Vec<Work> {
    let mut works: Vec<Work> = Vec::new();
    let mut open: HashMap<Option<&str>, usize> = HashMap::new();
    for turn in turns {
        let Some(row) = rows.get(turn.row) else { continue };
        let (committed, wrote) = (row.prompt.row.git_commits > 0, row.prompt.row.files_written > 0);
        let session = row.prompt.row.session.as_deref();
        if is_user(row) {
            open.insert(session, works.len());
            works.push(Work { prompt: turn.row, sent: turn.sent, ended: turn.ended, committed, wrote });
        } else if let Some(work) = open.get(&session).and_then(|&i| works.get_mut(i)) {
            work.ended = if turn.ended.at > work.ended.at { turn.ended } else { work.ended };
            work.committed |= committed;
            work.wrote |= wrote;
        }
    }
    works
}

/// When the change was made: its author time (an amend or a rebase keeps it), else its commit time.
fn made_at(commit: &Commit) -> Option<Timestamp> {
    Instant::parse(&commit.authored).or_else(|| Instant::parse(&commit.time)).map(|instant| instant.at)
}
