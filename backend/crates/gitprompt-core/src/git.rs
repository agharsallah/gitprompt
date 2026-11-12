//! Git facts for a working directory, read with the user's own `git` CLI (read-only commands, 2 s
//! timeout each). Port of `core/git.py`, plus a short-lived cache: a burst of prompts from one repo
//! runs git once (new in the Rust port).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use fancy_regex::{NoExpand, Regex};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Json, JsonMap};

/// How long one git command may run.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Field separator in `--format` strings.
const SEP: char = '\x1f';
/// Record separator in `history`'s `--format` string.
const RECORD: char = '\x1e';

/// Repository state when a prompt was sent; `{}` outside a repository, or when git is missing.
///
/// Keys: `git_root` (the *main* repository, also from a linked worktree), `git_worktree` +
/// `git_worktree_root` (only for a linked worktree; a bare main repository's root is the folder
/// itself), `git_branch`, `git_detached`, `git_head` (12 chars), `git_upstream`, `git_ahead`,
/// `git_behind`, `git_staged`, `git_modified`, `git_untracked`, `git_conflicts`, `git_remote`
/// (credentials stripped), `git_repo` (`owner/name`), `git_host`, `git_head_subject`,
/// `git_head_time`, `git_head_author`. Unknown values and false flags are left out; counts are kept
/// even when 0.
pub fn git_context(cwd: Option<&Path>) -> JsonMap {
    let Some(cwd) = cwd else { return JsonMap::new() };
    let Some(mut found) = layout(cwd) else { return JsonMap::new() };
    found.extend(status(cwd));
    found.extend(remote(cwd));
    found.extend(last_commit(cwd));
    found.retain(|_, value| !matches!(value, Json::Null | Json::Bool(false)) && value.as_str() != Some(""));
    found
}

/// stdout of `git -C cwd args...`, trimmed; `None` if git is missing, fails, times out (killed after
/// [`TIMEOUT`]) or prints nothing.
fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0") // read-only: never take the index lock from under the user
        .env("LC_ALL", "C") // untranslated output (`--shortstat` is parsed)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let output = read_with_timeout(stdout, TIMEOUT);
    if output.is_none() {
        let _ = child.kill(); // it may have exited meanwhile; nothing to do either way
    }
    let status = child.wait().ok()?;
    let text = String::from_utf8_lossy(&output?).trim().to_owned();
    (status.success() && !text.is_empty()).then_some(text)
}

/// Everything a reader yields until EOF, or `None` when that takes longer than `timeout`. Read on
/// its own thread, so a full pipe never blocks the command.
fn read_with_timeout(mut reader: impl Read + Send + 'static, timeout: Duration) -> Option<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let read = reader.read_to_end(&mut buf).map(|_| buf);
        let _ = tx.send(read); // the receiver is gone after a timeout
    });
    rx.recv_timeout(timeout).ok()?.ok()
}

/// The project (`git_root`) is the main repository, also from one of its linked worktrees: its prompts
/// belong to the same project. The worktree the prompt ran in is kept in `git_worktree_root`.
fn layout(cwd: &Path) -> Option<JsonMap> {
    let paths = git(cwd, &["rev-parse", "--path-format=absolute", "--show-toplevel", "--git-dir", "--git-common-dir"])?;
    let mut lines = paths.lines();
    let (top, git_dir, common) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""), lines.next().unwrap_or(""));
    let mut out = JsonMap::new();
    if git_dir == common || common.is_empty() {
        out.insert("git_root".into(), top.into());
        return Some(out);
    }
    let common = Path::new(common);
    // A linked worktree of a bare repository: the project is the bare folder itself.
    let main_root = match common.file_name() {
        Some(name) if name == ".git" => common.parent().unwrap_or(common),
        _ => common,
    };
    out.insert("git_root".into(), main_root.display().to_string().into());
    out.insert("git_worktree".into(), true.into());
    out.insert("git_worktree_root".into(), top.into());
    Some(out)
}

/// Branch, HEAD, upstream and working tree counts from `git status --porcelain=v2 --branch`.
fn status(cwd: &Path) -> JsonMap {
    let out = git(cwd, &["status", "--porcelain=v2", "--branch"]).unwrap_or_default();
    let mut headers: HashMap<&str, &str> = HashMap::new();
    let mut counts = Counts::default();
    for line in out.lines() {
        match line.strip_prefix("# ") {
            Some(header) => {
                if let Some((key, value)) = header.split_once(' ') {
                    headers.insert(key, value);
                }
            }
            None if !line.starts_with('#') => counts.add(line),
            None => {}
        }
    }
    let head = headers.get("branch.head").copied();
    let oid = headers.get("branch.oid").copied().unwrap_or("");
    let (ahead, behind) = headers.get("branch.ab").and_then(|ab| ahead_behind(ab)).unzip();
    let detached = head == Some("(detached)");
    let mut map = JsonMap::new();
    map.insert("git_branch".into(), head.filter(|_| !detached).into());
    map.insert("git_detached".into(), detached.into());
    map.insert("git_head".into(), (oid != "(initial)").then(|| oid.get(..12).unwrap_or(oid)).into());
    map.insert("git_upstream".into(), headers.get("branch.upstream").copied().into());
    map.insert("git_ahead".into(), ahead.into());
    map.insert("git_behind".into(), behind.into());
    map.insert("git_staged".into(), counts.staged.into());
    map.insert("git_modified".into(), counts.modified.into());
    map.insert("git_untracked".into(), counts.untracked.into());
    map.insert("git_conflicts".into(), counts.conflicts.into());
    map
}

/// `+1 -2` -> (1, 2).
fn ahead_behind(ab: &str) -> Option<(u64, u64)> {
    let mut parts = ab.split_whitespace().map(|part| part.get(1..)?.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??))
}

/// Working tree entry counts of a porcelain v2 status.
#[derive(Default)]
struct Counts {
    staged: u64,
    modified: u64,
    untracked: u64,
    conflicts: u64,
}

impl Counts {
    /// One entry line: `1 XY ...` / `2 XY ...` (changed / renamed), `u ...` (unmerged), `? path`.
    fn add(&mut self, entry: &str) {
        let bytes = entry.as_bytes();
        match bytes.first() {
            Some(b'1' | b'2') => {
                self.staged += u64::from(bytes.get(2).is_some_and(|&x| x != b'.'));
                self.modified += u64::from(bytes.get(3).is_some_and(|&y| y != b'.'));
            }
            Some(b'?') => self.untracked += 1,
            Some(b'u') => self.conflicts += 1,
            _ => {}
        }
    }
}

/// A regex from a constant pattern of this module; each is compiled by this module's tests, so this
/// cannot fail at run time.
#[expect(clippy::expect_used, reason = "constant patterns, each compiled by this module's tests")]
fn constant_regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("constant patterns in git.rs are valid")
}

static CREDENTIALS: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"//[^/@]+@"));
static REPO: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"[:/]([^/:]+/[^/]+?)(?:\.git)?/?$"));
static HOST: LazyLock<Regex> = LazyLock::new(|| constant_regex(r"(?:@|//)([^/:@]+)"));

fn remote(cwd: &Path) -> JsonMap {
    let mut map = JsonMap::new();
    let Some(url) = git(cwd, &["remote", "get-url", "origin"]) else { return map };
    // Never keep credentials embedded in a URL; if they can't be removed, report no remote at all.
    let Ok(url) = CREDENTIALS.try_replacen(&url, 0, NoExpand("//")) else { return map };
    map.insert("git_repo".into(), first_group(&REPO, &url).into());
    map.insert("git_host".into(), first_group(&HOST, &url).into());
    map.insert("git_remote".into(), url.into_owned().into());
    map
}

fn first_group(regex: &Regex, text: &str) -> Option<String> {
    let captures = regex.captures(text).ok()??;
    Some(captures.get(1)?.as_str().to_owned())
}

fn last_commit(cwd: &Path) -> JsonMap {
    let format = format!("--format=%s{SEP}%cI{SEP}%an");
    let log = git(cwd, &["log", "-1", &format]).unwrap_or_default();
    let mut fields = log.split(SEP);
    let mut map = JsonMap::new();
    map.insert("git_head_subject".into(), fields.next().into());
    map.insert("git_head_time".into(), fields.next().into());
    map.insert("git_head_author".into(), fields.next().into());
    map
}

/// One commit, as `commits` and `history` report it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "StoryCommit")]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CommitsAfter {
    /// Serialized as [`CommitHeader`]s: these commits carry no diffstat (the whole range's is in
    /// `diffstat`), so their zero counts would be noise on the wire.
    #[serde(skip_serializing_if = "Vec::is_empty", default, serialize_with = "commit_headers")]
    #[ts(as = "Vec<CommitHeader>")]
    pub commits: Vec<Commit>,
    /// `git diff --shortstat start end`, when `start` is known and there are commits.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub diffstat: Option<String>,
    /// The end of the range (the next prompt's head, else `HEAD`); empty, and left out, outside a
    /// repository.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub until: String,
}

/// A commit without its diffstat, as [`CommitsAfter`] writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "Commit")]
pub struct CommitHeader {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub time: String,
}

fn commit_headers<S: serde::Serializer>(commits: &[Commit], serializer: S) -> Result<S::Ok, S::Error> {
    let header =
        |c: &Commit| CommitHeader { hash: c.hash.clone(), subject: c.subject.clone(), author: c.author.clone(), time: c.time.clone() };
    serializer.collect_seq(commits.iter().map(header))
}

/// `start..end` when the head at prompt time is known, otherwise everything on `end` since `since`
/// (e.g. a repository that had no commits yet).
pub fn commits(root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
    let format = format!("--format=%h{SEP}%s{SEP}%an{SEP}%cI");
    let (range, since_arg) = (start.map(|start| format!("{start}..{end}")), format!("--since={since}"));
    let mut args = vec!["log", format.as_str()];
    match &range {
        Some(range) => args.push(range),
        None => args.extend([end, since_arg.as_str()]),
    }
    let log = git(root, &args).unwrap_or_default();
    let commits: Vec<Commit> = log.lines().map(commit_header).collect();
    let diffstat = match start {
        Some(start) if !commits.is_empty() => git(root, &["diff", "--shortstat", start, end]),
        _ => None,
    };
    CommitsAfter { commits, diffstat, until: end.to_owned() }
}

/// A commit from a `%h SEP %s SEP %an SEP %cI` line; missing fields are empty.
fn commit_header(line: &str) -> Commit {
    let mut fields = line.split(SEP).map(str::to_owned);
    let mut next = || fields.next().unwrap_or_default();
    Commit { hash: next(), subject: next(), author: next(), time: next(), ..Commit::default() }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortstat {
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

/// `{files, insertions, deletions}` from a `--shortstat` line; missing parts are 0.
pub fn shortstat(line: &str) -> Shortstat {
    let mut stat = Shortstat::default();
    for part in line.split(',') {
        let mut words = part.split_whitespace();
        let (Some(Ok(count)), Some(word)) = (words.next().map(str::parse::<u64>), words.next()) else { continue };
        match word.trim_end_matches(|c: char| !c.is_ascii_alphabetic()) {
            "file" | "files" => stat.files = count,
            "insertion" | "insertions" => stat.insertions = count,
            "deletion" | "deletions" => stat.deletions = count,
            _ => {}
        }
    }
    stat
}

/// Commits on local branches with a commit time in `[since, until]`, oldest first, each with its
/// diffstat: one git call for a whole story. `[]` outside a repository or if git fails.
pub fn history(root: &Path, since: &str, until: &str) -> Vec<Commit> {
    let (since, until) = (format!("--since={since}"), format!("--until={until}"));
    let format = format!("--format={RECORD}%h{SEP}%s{SEP}%an{SEP}%cI");
    let log = git(root, &["log", "--branches", &since, &until, &format, "--shortstat"]).unwrap_or_default();
    let mut found: Vec<Commit> = log.split(RECORD).filter_map(history_record).collect();
    found.reverse();
    found
}

/// A header line, then (after a blank line) its `--shortstat` line, if the commit changed files.
fn history_record(record: &str) -> Option<Commit> {
    let (header, rest) = record.trim().split_once('\n').unwrap_or_else(|| (record.trim(), ""));
    if header.is_empty() {
        return None;
    }
    let Shortstat { files, insertions, deletions } = shortstat(rest);
    Some(Commit { files, insertions, deletions, ..commit_header(header) })
}

/// [`git_context`] memoized per directory for `ttl`. Thread-safe.
///
/// Git runs outside the lock, so a slow repository never blocks lookups for others; two threads
/// missing on the same directory at once may both run git (the later result wins).
#[derive(Debug)]
pub struct GitCache {
    ttl: Duration,
    entries: Mutex<HashMap<PathBuf, Entry>>,
    misses: AtomicU64,
}

#[derive(Debug)]
struct Entry {
    at: Instant,
    context: JsonMap,
}

impl GitCache {
    pub fn new(ttl: Duration) -> Self {
        GitCache { ttl, entries: Mutex::new(HashMap::new()), misses: AtomicU64::new(0) }
    }

    /// [`git_context`] of `cwd`, from the cache while younger than the ttl. `None` is `{}` (no git run).
    pub fn context(&self, cwd: Option<&Path>) -> JsonMap {
        let Some(cwd) = cwd else { return JsonMap::new() };
        if let Some(context) = self.fresh(cwd) {
            return context;
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let context = git_context(Some(cwd));
        let mut entries = self.lock();
        let ttl = self.ttl;
        entries.retain(|_, entry| entry.at.elapsed() < ttl); // keeps the map as small as the burst
        entries.insert(cwd.to_path_buf(), Entry { at: Instant::now(), context: context.clone() });
        context
    }

    /// How many times git was actually run for a context (observability and tests).
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    fn fresh(&self, cwd: &Path) -> Option<JsonMap> {
        let entries = self.lock();
        let entry = entries.get(cwd)?;
        (entry.at.elapsed() < self.ttl).then(|| entry.context.clone())
    }

    /// The entries; a panic while they were held can't leave them half-updated (each change is one
    /// `insert`/`retain`), so a poisoned lock is safe to use.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_patterns_compile() {
        for regex in [&CREDENTIALS, &REPO, &HOST] {
            assert!(regex.is_match("x").is_ok());
        }
    }

    #[test]
    fn ahead_behind_reads_both_counts() {
        assert_eq!(ahead_behind("+3 -0"), Some((3, 0)));
        assert_eq!(ahead_behind("+3"), None);
        assert_eq!(ahead_behind("garbage here"), None);
    }

    #[test]
    fn porcelain_entries_are_counted_by_kind() {
        let mut counts = Counts::default();
        for line in ["1 M. N... a", "1 .M N... b", "2 RM N... c d", "? e", "u UU N... f", "! ignored"] {
            counts.add(line);
        }
        assert_eq!((counts.staged, counts.modified, counts.untracked, counts.conflicts), (2, 2, 1, 1));
    }
}
