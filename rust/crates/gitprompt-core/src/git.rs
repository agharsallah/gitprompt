//! Git facts for a working directory, read with the user's own `git` CLI (read-only commands, 2 s
//! timeout each). Port of `core/git.py`, plus a short-lived cache: a burst of prompts from one repo
//! runs git once (new in the Rust port).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::JsonMap;

/// Repository state when a prompt was sent; `{}` outside a repository, or when git is missing.
///
/// Keys: `git_root` (the *main* repository, also from a linked worktree), `git_worktree` +
/// `git_worktree_root` (only for a linked worktree; a bare main repository's root is the folder
/// itself), `git_branch`, `git_detached`, `git_head` (12 chars), `git_upstream`, `git_ahead`,
/// `git_behind`, `git_staged`, `git_modified`, `git_untracked`, `git_conflicts`, `git_remote`
/// (credentials stripped), `git_repo` (`owner/name`), `git_host`, `git_head_subject`,
/// `git_head_time`, `git_head_author`. Unknown values and false flags are left out; counts are kept
/// even when 0.
pub fn git_context(_cwd: Option<&Path>) -> JsonMap {
    todo!()
}

/// One commit, as `commits` and `history` report it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    /// Committer time, ISO-8601.
    pub time: String,
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub insertions: u64,
    #[serde(default)]
    pub deletions: u64,
}

/// Commits made after a prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitsAfter {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub commits: Vec<Commit>,
    /// `git diff --shortstat start end`, when `start` is known and there are commits.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diffstat: Option<String>,
    pub until: String,
}

/// `start..end` when the head at prompt time is known, otherwise everything on `end` since `since`
/// (e.g. a repository that had no commits yet).
pub fn commits(_root: &Path, _start: Option<&str>, _end: &str, _since: &str) -> CommitsAfter {
    todo!()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortstat {
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

/// `{files, insertions, deletions}` from a `--shortstat` line; missing parts are 0.
pub fn shortstat(_line: &str) -> Shortstat {
    todo!()
}

/// Commits on local branches with a commit time in `[since, until]`, oldest first, each with its
/// diffstat: one git call for a whole story. `[]` outside a repository or if git fails.
pub fn history(_root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
    todo!()
}

/// [`git_context`] memoized per directory for `ttl`. Thread-safe.
pub struct GitCache {
    _ttl: Duration,
}

impl GitCache {
    pub fn new(ttl: Duration) -> Self {
        GitCache { _ttl: ttl }
    }

    pub fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        todo!()
    }

    /// How many times git was actually run for a context (observability and tests).
    pub fn misses(&self) -> u64 {
        todo!()
    }
}
