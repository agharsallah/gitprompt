//! [`history`] kept between reads. Listing a repository's commits with what each changed is the
//! slowest git call there is (most of a second on a few hundred commits), and the palette asks for
//! it in every recent project on every keystroke. The list can only change when a local branch
//! moves, so it is kept per repository and window start, and read again only then.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::{Commit, git, history};

/// Lists kept at most; the one checked longest ago goes first.
const KEPT: usize = 256;

/// A repository and the start of a window.
type Key = (PathBuf, String);

#[derive(Debug)]
struct Kept {
    /// When the branches were last seen where `heads` says.
    checked: Instant,
    /// Every local branch with the commit it is on.
    heads: String,
    /// The end of the window that was read.
    until: String,
    commits: Vec<Commit>,
}

/// [`history`] memoized per repository and `since`. Thread-safe.
#[derive(Debug)]
pub struct HistoryCache {
    ttl: Duration,
    entries: Mutex<HashMap<Key, Kept>>,
    misses: AtomicU64,
}

impl HistoryCache {
    /// `ttl`: how long a list is served without asking git whether a branch moved.
    pub fn new(ttl: Duration) -> Self {
        HistoryCache { ttl, entries: Mutex::new(HashMap::new()), misses: AtomicU64::new(0) }
    }

    /// [`history`] of `root` in `[since, until]`. A kept list answers while no local branch has
    /// moved since it was read and it was read up to `until` or, both ends being UTC, up to an
    /// earlier time: without a new commit on a branch, a later end lists nothing more.
    pub fn history(&self, root: &Path, since: &str, until: &str) -> Vec<Commit> {
        let key = (root.to_path_buf(), since.to_owned());
        if let Some(commits) = self.kept(&key, until, None) {
            return commits;
        }
        let Some(heads) = branch_heads(root) else { return history(root, since, until) };
        if let Some(commits) = self.kept(&key, until, Some(&heads)) {
            return commits;
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let commits = history(root, since, until);
        // An empty list is also what a git that failed or ran out of time gives: never kept.
        if !commits.is_empty() {
            self.keep(key, Kept { checked: Instant::now(), heads, until: until.to_owned(), commits: commits.clone() });
        }
        commits
    }

    /// How many times the commits were actually listed (observability and tests).
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// The kept list for `key` when it answers a read up to `until`: checked within the ttl, or
    /// (given what the branches are on now) still on the same `heads`.
    fn kept(&self, key: &Key, until: &str, heads: Option<&str>) -> Option<Vec<Commit>> {
        let mut entries = self.lock();
        let kept = entries.get_mut(key).filter(|kept| covers(&kept.until, until))?;
        match heads {
            None if kept.checked.elapsed() < self.ttl => {}
            Some(heads) if kept.heads == heads => kept.checked = Instant::now(),
            _ => return None,
        }
        Some(kept.commits.clone())
    }

    fn keep(&self, key: Key, kept: Kept) {
        let mut entries = self.lock();
        if entries.len() >= KEPT
            && !entries.contains_key(&key)
            && let Some(oldest) = entries.iter().min_by_key(|(_, kept)| kept.checked).map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(key, kept);
    }

    /// The entries; each change is one `insert` or `remove`, so a poisoned lock is safe to use.
    fn lock(&self) -> MutexGuard<'_, HashMap<Key, Kept>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Every local branch with the commit it is on: what [`history`] (`git log --branches`) depends on.
/// `None` outside a repository, or in one without a branch.
fn branch_heads(root: &Path) -> Option<String> {
    git(root, &["for-each-ref", "--format=%(objectname) %(refname)", "refs/heads"])
}

/// Whether a list read up to `read` answers a read up to `asked`: the same end, or a later one
/// when both are UTC to the second or finer (`2026-01-02T10:00:00Z`, which sort as text).
fn covers(read: &str, asked: &str) -> bool {
    /// `YYYY-MM-DDTHH:MM:SS`.
    const TO_THE_SECOND: usize = 19;
    let utc = |time: &str| time.ends_with('Z').then(|| time.get(..TO_THE_SECOND)).flatten().map(str::to_owned);
    read == asked || matches!((utc(read), utc(asked)), (Some(read), Some(asked)) if read <= asked)
}

#[cfg(test)]
mod tests {
    use super::covers;

    #[test]
    fn a_list_answers_the_same_end_or_a_later_utc_one() {
        assert!(covers("2026-01-02T10:00:00+02:00", "2026-01-02T10:00:00+02:00"));
        assert!(covers("2026-01-02T10:00:00Z", "2026-01-02T10:00:05.250Z"));
        assert!(covers("2026-01-02T10:00:00.9Z", "2026-01-02T10:00:00Z"));
        assert!(!covers("2026-01-02T10:00:05Z", "2026-01-02T10:00:00Z"));
        assert!(!covers("2026-01-02T10:00:00+02:00", "2026-01-02T12:00:00+02:00"));
        assert!(!covers("2026-01-02T10:00:00Z", "2026-01-02T12:00:00+02:00"));
    }
}
