//! A commit by its hash, from anywhere (the terminal, ⌘K, a link): its trace in the project that
//! has it, and its page on the forge when the repository has a known remote.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CommitTrace, ProjectRef, ProjectTraces, names_hash};
use crate::time::Instant;

/// A commit found by its hash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct FoundCommit {
    #[serde(flatten)]
    pub trace: CommitTrace,
    /// The project whose commits list it.
    pub project: ProjectRef,
}

/// What a hash names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "found", rename_all = "snake_case")]
pub enum CommitLookup {
    /// One commit.
    One { commit: Box<FoundCommit> },
    /// A prefix of several (in one project or across projects), newest first: say which.
    Many { candidates: Vec<FoundCommit> },
}

/// The commits of `projects` that `hash` names ([`names_hash`]), each with its URL on the forge of
/// `remote_of(repo)` (the `origin` URL, [`CommitTrace::on_remote`]); `None` when it names none.
pub fn lookup_commit(hash: &str, projects: &[ProjectTraces], remote_of: impl Fn(&str) -> Option<String>) -> Option<CommitLookup> {
    let mut found: Vec<FoundCommit> = projects
        .iter()
        .flat_map(|p| p.traces.iter().filter(|t| names_hash(&t.commit.hash, hash)).map(move |t| (p, t)))
        .map(|(p, trace)| FoundCommit { trace: trace.clone().on_remote(remote_of(&p.project.repo).as_deref()), project: p.project.clone() })
        .collect();
    found.sort_by_key(|f| Reverse(Instant::parse(&f.trace.commit.time).map(|time| time.at)));
    match found.len() {
        0 => None,
        1 => found.pop().map(|commit| CommitLookup::One { commit: Box::new(commit) }),
        _ => Some(CommitLookup::Many { candidates: found }),
    }
}
