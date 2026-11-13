//! The spool's files, and the locks that let appends, drains and readers (`gitprompt doctor`, the
//! daemon's status) share them across processes without ever seeing half a change:
//!
//! - an append holds an exclusive lock on the queue while it writes ([`open_append`]);
//! - a drain holds [`DrainLock`] from start to end, so two drains (the daemon's at start and
//!   `gitprompt spool drain`) never hand the same line over twice: the second waits, then finds
//!   only what the first left;
//! - the drain's progress is rewritten under an exclusive lock and read under a shared one
//!   ([`Progress`], [`read_offset`]), so a reader never sees a half-written number.
//!
//! Waiting on a lock is the only coordination: nothing here depends on how long anything takes.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use gitprompt_core::Result;

use crate::installer::create_private_dir;

/// Mode of every spool file: payloads hold prompts.
const FILE_MODE: u32 = 0o600;
/// The lock file a drain holds, in the spool directory (no source has this name: it has a dot).
pub const DRAIN_LOCK: &str = "drain.lock";

/// Held while one drain runs; released when dropped (or when the process dies: an OS lock).
#[derive(Debug)]
pub(super) struct DrainLock {
    /// Only held: closing it releases the lock.
    _held: File,
}

impl DrainLock {
    /// Wait for any other drain of this spool directory to finish, then hold the lock.
    pub(super) fn take(dir: &Path) -> Result<Self> {
        create_private_dir(dir)?;
        let file = open(dir.join(DRAIN_LOCK).as_path(), OpenOptions::new().write(true).create(true).truncate(false))?;
        file.lock()?;
        Ok(DrainLock { _held: file })
    }
}

/// The progress file of a drain, kept open and rewritten in place after each line: a fixed-width
/// number (wider than any earlier value, so no stale digit is left after it), never empty or
/// half-replaced the way a crash between a truncate and a write, or an unflushed rename, could leave
/// it. Not flushed to disk: see the module docs of [`super`].
pub(super) struct Progress(File);

impl Progress {
    /// Open (created with mode 0600) and record `offset`, the progress so far.
    pub(super) fn open(path: &Path, offset: u64) -> Result<Self> {
        let mut progress = Progress(open(path, OpenOptions::new().write(true).create(true).truncate(false))?);
        progress.record(offset)?;
        Ok(progress)
    }

    /// Rewrite the progress, holding the file's exclusive lock meanwhile ([`read_offset`] holds a
    /// shared one).
    pub(super) fn record(&mut self, offset: u64) -> Result<()> {
        self.0.lock()?;
        let written = self.0.seek(SeekFrom::Start(0)).and_then(|_| self.0.write_all(format!("{offset:020}").as_bytes()));
        self.0.unlock()?;
        Ok(written?)
    }
}

/// The recorded progress of a drain, read under a shared lock; 0 when there is none (or it is
/// unreadable: re-reading from the start re-ingests, which the prompt-id dedup absorbs, rather than
/// skipping payloads).
pub(super) fn read_offset(path: &Path) -> Result<u64> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    file.lock_shared()?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    drop(file); // releases the lock
    Ok(text.trim().parse().unwrap_or_else(|_| {
        tracing::warn!(path = %path.display(), "unreadable drain offset; draining from the start");
        0
    }))
}

/// Open for appending, creating it with mode 0600.
pub(super) fn open_append(path: &Path) -> io::Result<File> {
    open(path, OpenOptions::new().append(true).create(true))
}

/// Open with `options`, a file created with mode 0600.
fn open(path: &Path, options: &mut OpenOptions) -> io::Result<File> {
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(options, FILE_MODE);
    options.open(path)
}

/// Whether the open `file` is the one at `path` now.
#[cfg(unix)]
pub(super) fn is_same_file(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let open = file.metadata()?;
    match std::fs::metadata(path) {
        Ok(now) => Ok(open.dev() == now.dev() && open.ino() == now.ino()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
pub(super) fn is_same_file(_file: &File, path: &Path) -> io::Result<bool> {
    Ok(path.exists())
}

/// Make sure there is a draining file: what an interrupted drain left, else the queue moved aside
/// (hooks then append to a new queue). `false` when there is neither.
pub(super) fn take_queue(queue: &Path, draining: &Path) -> io::Result<bool> {
    if draining.exists() {
        return Ok(true);
    }
    match std::fs::rename(queue, draining) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// The non-blank lines of a file from byte `offset`, each with the offset just after it. A missing
/// file has none.
pub(super) fn read_lines(path: &Path, offset: u64) -> Result<Vec<(String, u64)>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let start = usize::try_from(offset).unwrap_or(usize::MAX).min(bytes.len());
    let mut lines = Vec::new();
    let mut at = start;
    for chunk in bytes.get(start..).unwrap_or_default().split_inclusive(|b| *b == b'\n') {
        at += chunk.len();
        let line = String::from_utf8_lossy(chunk);
        let line = line.trim_end_matches(['\n', '\r']);
        if !line.trim().is_empty() {
            lines.push((line.to_owned(), at as u64));
        }
    }
    Ok(lines)
}

pub(super) fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_carry_the_offset_after_them_and_blank_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("q.jsonl");
        std::fs::write(&path, "a\n\nbc\nd").unwrap();
        assert_eq!(read_lines(&path, 0).unwrap(), vec![("a".into(), 2), ("bc".into(), 6), ("d".into(), 7)]);
        assert_eq!(read_lines(&path, 3).unwrap(), vec![("bc".into(), 6), ("d".into(), 7)]);
    }

    #[test]
    fn progress_is_rewritten_in_place_and_read_back_over_an_older_shorter_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.draining.offset");
        std::fs::write(&path, "7").unwrap(); // as an earlier version wrote it
        assert_eq!(read_offset(&path).unwrap(), 7);
        let mut progress = Progress::open(&path, 7).unwrap();
        progress.record(1234).unwrap();
        progress.record(99_999).unwrap();
        assert_eq!(read_offset(&path).unwrap(), 99_999);
        assert_eq!(read_offset(&dir.path().join("none")).unwrap(), 0);
    }
}
