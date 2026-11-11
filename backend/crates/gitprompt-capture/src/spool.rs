//! The offline queue (new in the Rust port). When the daemon can't be reached, the hook forwarder
//! appends the payload to `<home>/spool/<source>.jsonl` instead of losing it, one compact JSON line
//! per payload: `{"spooled_at": "<UTC ISO-8601>", "payload": {...}}`. At start-up the daemon drains
//! every file into the pipeline, oldest line first, and stores each payload as received at its
//! `spooled_at` (a bare payload line, as an earlier forwarder wrote it, is received at drain time):
//!
//! - an ingested line is removed (a crash mid-drain re-ingests at most the line in flight, which the
//!   prompt-id dedup absorbs for prompts);
//! - a line that is not valid JSON (or not an object) is moved, as is, to `<source>.corrupt.jsonl`
//!   next to it and the drain goes on;
//! - when ingesting a line fails (e.g. the store), the drain stops there and that line and every later
//!   one stay queued, in order, for the next start;
//! - a fully drained file is deleted; a file whose source has no spec is left alone.
//!
//! Appends and drains may run at the same time (a hook fires while the daemon starts). A drain first
//! renames the queue to `<source>.draining.jsonl`, so new payloads start a fresh queue, and records
//! its progress in `<source>.draining.offset` (bytes consumed). Each append holds an exclusive lock
//! on the file while it writes and checks that the file it locked is still the queue, so no payload
//! is ever written into a file the drain has already read.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use gitprompt_core::files::atomic_write;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::Spec;
use gitprompt_core::{Error, Json, Result, Settings};
use gitprompt_store::Store;
use gitprompt_store::store::Clock;

use crate::installer::create_private_dir;
use crate::pipeline::{CaptureOptions, Captured, Observer, capture_at, iso};

/// Suffix of a queue file.
pub const QUEUE_SUFFIX: &str = ".jsonl";
/// Suffix of the file corrupted lines are moved to.
pub const CORRUPT_SUFFIX: &str = ".corrupt.jsonl";
/// Suffix of a queue being drained (renamed from the queue file).
pub const DRAINING_SUFFIX: &str = ".draining.jsonl";
/// Suffix of the progress of a drain: bytes of the draining file already handled.
pub const OFFSET_SUFFIX: &str = ".draining.offset";

/// Mode of every spool file: payloads hold prompts.
const FILE_MODE: u32 = 0o600;

/// What one drain did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drained {
    /// Lines handed to the pipeline successfully (and removed).
    pub ingested: usize,
    /// Lines moved aside to the corrupt file.
    pub corrupted: usize,
    /// Lines still queued (the one that failed and those after it).
    pub remaining: usize,
}

/// One queued payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Spooled {
    pub payload: Json,
    /// When the forwarder queued it; `None` for a bare payload line (written before envelopes).
    pub spooled_at: Option<Timestamp>,
}

impl Spooled {
    /// A queue line: an envelope `{"spooled_at", "payload"}`, or a bare payload object. `None` for a
    /// line that is not valid JSON or not an object (it could never be ingested).
    pub fn parse(line: &str) -> Option<Spooled> {
        let Json::Object(mut map) = serde_json::from_str::<Json>(line).ok()? else { return None };
        let envelope =
            map.len() == 2 && map.get("payload").is_some_and(Json::is_object) && map.get("spooled_at").is_some_and(Json::is_string);
        if !envelope {
            return Some(Spooled { payload: Json::Object(map), spooled_at: None });
        }
        // A time that doesn't parse is not worth losing the payload over: it is received at drain time.
        let spooled_at = map.get("spooled_at").and_then(Json::as_str).and_then(|t| t.parse::<Timestamp>().ok());
        Some(Spooled { payload: map.remove("payload")?, spooled_at })
    }

    /// The queue line of a payload spooled at `at`.
    fn line(payload: &Json, at: Timestamp) -> Result<String> {
        let envelope = serde_json::json!({"spooled_at": iso(at), "payload": payload});
        Ok(serde_json::to_string(&envelope)?)
    }
}

/// A spool directory.
#[derive(Clone)]
pub struct Spool {
    pub dir: PathBuf,
    /// Stamps `spooled_at` on appended payloads.
    clock: Clock,
}

impl std::fmt::Debug for Spool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spool").field("dir", &self.dir).finish_non_exhaustive()
    }
}

/// Two spools are the same when they use the same directory (the clock is not compared).
impl PartialEq for Spool {
    fn eq(&self, other: &Self) -> bool {
        self.dir == other.dir
    }
}

impl Eq for Spool {}

impl Spool {
    /// A spool in `dir`, stamping payloads with the system clock.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Spool { dir: dir.into(), clock: Arc::new(Timestamp::now) }
    }

    /// `Settings::spool_dir()`.
    pub fn of(settings: &Settings) -> Self {
        Self::new(settings.spool_dir())
    }

    /// The same spool stamping payloads with `clock` (tests).
    #[must_use]
    pub fn with_clock(self, clock: Clock) -> Self {
        Spool { clock, ..self }
    }

    /// `<dir>/<source>.jsonl`. `Error::Invalid` for a source id that isn't a plain name
    /// (`^[a-z0-9][a-z0-9-]*$`), so a payload can never be written outside the directory.
    pub fn queue_file(&self, source: &str) -> Result<PathBuf> {
        self.file(source, QUEUE_SUFFIX)
    }

    /// `<dir>/<source>.corrupt.jsonl` (same validation as [`Self::queue_file`]).
    pub fn corrupt_file(&self, source: &str) -> Result<PathBuf> {
        self.file(source, CORRUPT_SUFFIX)
    }

    fn draining_file(&self, source: &str) -> Result<PathBuf> {
        self.file(source, DRAINING_SUFFIX)
    }

    fn offset_file(&self, source: &str) -> Result<PathBuf> {
        self.file(source, OFFSET_SUFFIX)
    }

    fn file(&self, source: &str, suffix: &str) -> Result<PathBuf> {
        if !is_plain_name(source) {
            return Err(Error::Invalid(format!("{source:?} is not a source id (a-z, 0-9 and -)")));
        }
        Ok(self.dir.join(format!("{source}{suffix}")))
    }

    /// Append one payload, stamped with the spool's clock, as a single line (directory created with
    /// mode 0700, file mode 0600).
    pub fn append(&self, source: &str, payload: &Json) -> Result<()> {
        self.append_at(source, payload, (self.clock)())
    }

    /// [`Self::append`] with an explicit `spooled_at`.
    pub fn append_at(&self, source: &str, payload: &Json, spooled_at: Timestamp) -> Result<()> {
        let path = self.queue_file(source)?;
        let mut line = Spooled::line(payload, spooled_at)?;
        line.push('\n');
        create_private_dir(&self.dir)?;
        loop {
            let mut file = open_append(&path)?;
            file.lock()?;
            // A drain may have renamed the queue between our open and our lock: write only into
            // the file that is still the queue, else retry with the new one.
            if is_same_file(&file, &path)? {
                file.write_all(line.as_bytes())?;
                return Ok(file.flush()?);
            }
        }
    }

    /// Sources with a non-empty queue, sorted. A missing directory = none.
    pub fn sources(&self) -> Result<Vec<String>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut names = Vec::new();
        for entry in entries {
            let name = entry?.file_name().to_string_lossy().into_owned();
            let source = [DRAINING_SUFFIX, QUEUE_SUFFIX].iter().find_map(|suffix| name.strip_suffix(suffix));
            if let Some(source) = source.filter(|s| is_plain_name(s)) {
                names.push(source.to_owned());
            }
        }
        names.sort();
        names.dedup();
        let mut sources = Vec::new();
        for source in names {
            if !self.pending(&source)?.is_empty() {
                sources.push(source);
            }
        }
        Ok(sources)
    }

    /// Queued lines of a source (not yet ingested), oldest first, as stored.
    pub fn pending(&self, source: &str) -> Result<Vec<String>> {
        let offset = read_offset(&self.offset_file(source)?)?;
        let mut lines = read_lines(&self.draining_file(source)?, offset)?;
        lines.extend(read_lines(&self.queue_file(source)?, 0)?);
        Ok(lines.into_iter().map(|(line, _)| line).collect())
    }

    /// Hand each queued payload of `source` to `ingest`, oldest first (see the module docs).
    pub fn drain(&self, source: &str, ingest: &mut dyn FnMut(Json) -> Result<()>) -> Result<Drained> {
        self.drain_spooled(source, &mut |spooled| ingest(spooled.payload))
    }

    /// [`Self::drain`], handing over each payload with the time it was spooled.
    pub fn drain_spooled(&self, source: &str, ingest: &mut dyn FnMut(Spooled) -> Result<()>) -> Result<Drained> {
        let (queue, draining) = (self.queue_file(source)?, self.draining_file(source)?);
        let mut drained = Drained::default();
        // At most two passes (what an interrupted drain left, or the queue, then the queue as it is
        // after that), so hooks spooling while the daemon starts can't keep the drain going forever;
        // anything appended later waits for the next start.
        for _ in 0..2 {
            if !draining.exists() {
                match std::fs::rename(&queue, &draining) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(e) => return Err(e.into()),
                }
            }
            if !self.drain_file(source, ingest, &mut drained)? {
                break;
            }
        }
        drained.remaining = self.pending(source)?.len();
        Ok(drained)
    }

    /// Drain `<source>.draining.jsonl` from its recorded offset. `true` when it was drained whole
    /// (and deleted); `false` when an ingest failed (the file and offset stay, at the failed line).
    fn drain_file(&self, source: &str, ingest: &mut dyn FnMut(Spooled) -> Result<()>, drained: &mut Drained) -> Result<bool> {
        let (draining, offset_file) = (self.draining_file(source)?, self.offset_file(source)?);
        // Wait for an append that locked the file before it was renamed.
        File::open(&draining)?.lock()?;
        for (line, end) in read_lines(&draining, read_offset(&offset_file)?)? {
            if let Some(spooled) = Spooled::parse(&line) {
                match ingest(spooled) {
                    Ok(()) => drained.ingested += 1,
                    Err(e) => {
                        tracing::warn!(%source, error = %e, "spooled payload not ingested; it stays queued");
                        return Ok(false);
                    }
                }
            } else {
                self.set_aside(source, &line)?;
                drained.corrupted += 1;
            }
            atomic_write(&offset_file, &end.to_string(), FILE_MODE)?;
        }
        std::fs::remove_file(&draining)?;
        remove_if_exists(&offset_file)?;
        Ok(true)
    }

    /// Append a line that can't be ingested to the corrupt file, as it was queued.
    fn set_aside(&self, source: &str, line: &str) -> Result<()> {
        tracing::warn!(%source, "a spooled line is not a JSON object; moved to the corrupt file");
        let mut file = open_append(&self.corrupt_file(source)?)?;
        file.write_all(format!("{line}\n").as_bytes())?;
        Ok(())
    }
}

/// Drain every queue into the capture pipeline at start-up, with the options each source has under
/// `preferences` (a turned-off source is drained and discarded, as it would have been live). Each
/// payload is stored as received at the time it was spooled. Sources without a spec are skipped and
/// keep their queue; a source whose queue can't be read is reported in the log and keeps its queue
/// too, so one broken file never holds up the others. Returns per source what was drained, and what
/// the pipeline stored overall (so the caller marks turns for the index).
pub fn drain_into_pipeline(
    spool: &Spool,
    specs: &BTreeMap<String, Arc<Spec>>,
    store: &Store,
    preferences: &Preferences,
    observer: &Observer<'_>,
) -> Result<(BTreeMap<String, Drained>, Captured)> {
    let mut report = BTreeMap::new();
    let mut total = Captured::default();
    for source in spool.sources()? {
        let Some(spec) = specs.get(&source) else {
            tracing::info!(%source, "spooled payloads of a source without a spec are kept");
            continue;
        };
        let options = CaptureOptions::for_source(preferences, &source);
        let mut ingest = |spooled: Spooled| {
            total.absorb(capture_at(spec, store, &spooled.payload, options, observer, spooled.spooled_at)?);
            Ok(())
        };
        match spool.drain_spooled(&source, &mut ingest) {
            Ok(drained) => {
                report.insert(source, drained);
            }
            Err(e) => tracing::warn!(%source, error = %e, "could not drain the spool"),
        }
    }
    Ok((report, total))
}

/// `^[a-z0-9][a-z0-9-]*$`: safe as a file name and in a shell command.
pub(crate) fn is_plain_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Open for appending, creating it with mode 0600.
fn open_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, FILE_MODE);
    options.open(path)
}

/// Whether the open `file` is the one at `path` now.
#[cfg(unix)]
fn is_same_file(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let open = file.metadata()?;
    match std::fs::metadata(path) {
        Ok(now) => Ok(open.dev() == now.dev() && open.ino() == now.ino()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
fn is_same_file(_file: &File, path: &Path) -> io::Result<bool> {
    Ok(path.exists())
}

/// The non-blank lines of a file from byte `offset`, each with the offset just after it. A missing
/// file has none.
fn read_lines(path: &Path, offset: u64) -> Result<Vec<(String, u64)>> {
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

/// The recorded progress of a drain; 0 when there is none (or it is unreadable: re-reading from the
/// start re-ingests, which the prompt-id dedup absorbs, rather than skipping payloads).
fn read_offset(path: &Path) -> Result<u64> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text.trim().parse().unwrap_or_else(|_| {
            tracing::warn!(path = %path.display(), "unreadable drain offset; draining from the start");
            0
        })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e.into()),
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_names_are_lowercase_ids() {
        assert!(is_plain_name("claude-code") && is_plain_name("0x"));
        assert!(!is_plain_name("") && !is_plain_name("-x") && !is_plain_name("a/b") && !is_plain_name("A") && !is_plain_name("a.b"));
    }

    #[test]
    fn an_envelope_gives_its_payload_and_time_and_a_bare_object_is_its_own_payload() {
        let at: Timestamp = "2026-11-07T10:00:00Z".parse().unwrap();
        let line = Spooled::line(&json!({"a": 1}), at).unwrap();
        assert_eq!(Spooled::parse(&line), Some(Spooled { payload: json!({"a": 1}), spooled_at: Some(at) }));
        assert_eq!(Spooled::parse(r#"{"a": 1}"#), Some(Spooled { payload: json!({"a": 1}), spooled_at: None }));
        assert_eq!(Spooled::parse("[1]"), None);
        assert_eq!(Spooled::parse("{\"half\": "), None);
    }

    #[test]
    fn lines_carry_the_offset_after_them_and_blank_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("q.jsonl");
        std::fs::write(&path, "a\n\nbc\nd").unwrap();
        assert_eq!(read_lines(&path, 0).unwrap(), vec![("a".into(), 2), ("bc".into(), 6), ("d".into(), 7)]);
        assert_eq!(read_lines(&path, 3).unwrap(), vec![("bc".into(), 6), ("d".into(), 7)]);
    }
}
