//! The offline queue (new in the Rust port). When the daemon can't be reached, the hook forwarder
//! appends the payload to `<home>/spool/<source>.jsonl` (one compact JSON object per line) instead of
//! losing it. At start-up the daemon drains every file into the pipeline, oldest line first:
//!
//! - an ingested line is removed (a crash mid-drain re-ingests at most the line in flight, which the
//!   prompt-id dedup absorbs for prompts);
//! - a line that is not valid JSON is moved, as is, to `<source>.corrupt.jsonl` next to it and the
//!   drain goes on;
//! - when ingesting a line fails (e.g. the store), the drain stops there and that line and every later
//!   one stay queued, in order, for the next start;
//! - a fully drained file is deleted; a file whose source has no spec is left alone.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::Spec;
use gitprompt_core::{Json, Result, Settings};
use gitprompt_store::Store;

use crate::pipeline::{Captured, Observer};

/// Suffix of a queue file.
pub const QUEUE_SUFFIX: &str = ".jsonl";
/// Suffix of the file corrupted lines are moved to.
pub const CORRUPT_SUFFIX: &str = ".corrupt.jsonl";

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

/// A spool directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spool {
    pub dir: PathBuf,
}

impl Spool {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Spool { dir: dir.into() }
    }

    /// `Settings::spool_dir()`.
    pub fn of(_settings: &Settings) -> Self {
        todo!()
    }

    /// `<dir>/<source>.jsonl`. `Error::Invalid` for a source id that isn't a plain name
    /// (`^[a-z0-9][a-z0-9-]*$`), so a payload can never be written outside the directory.
    pub fn queue_file(&self, _source: &str) -> Result<PathBuf> {
        todo!()
    }

    /// `<dir>/<source>.corrupt.jsonl` (same validation as [`Self::queue_file`]).
    pub fn corrupt_file(&self, _source: &str) -> Result<PathBuf> {
        todo!()
    }

    /// Append one payload as a single line (directory created with mode 0700, file mode 0600).
    pub fn append(&self, _source: &str, _payload: &Json) -> Result<()> {
        todo!()
    }

    /// Sources with a non-empty queue, sorted. A missing directory = none.
    pub fn sources(&self) -> Result<Vec<String>> {
        todo!()
    }

    /// Queued lines of a source (not yet ingested), oldest first, as stored.
    pub fn pending(&self, _source: &str) -> Result<Vec<String>> {
        todo!()
    }

    /// Hand each queued payload of `source` to `ingest`, oldest first (see the module docs).
    pub fn drain(&self, _source: &str, _ingest: &mut dyn FnMut(Json) -> Result<()>) -> Result<Drained> {
        todo!()
    }
}

/// Drain every queue into the capture pipeline at start-up, with the options each source has under
/// `preferences` (a turned-off source is drained and discarded, as it would have been live). Sources
/// without a spec are skipped and keep their queue. Returns per source what was drained, and what the
/// pipeline stored overall (so the caller marks turns for the index).
pub fn drain_into_pipeline(
    _spool: &Spool,
    _specs: &BTreeMap<String, Arc<Spec>>,
    _store: &Store,
    _preferences: &Preferences,
    _observer: &Observer<'_>,
) -> Result<(BTreeMap<String, Drained>, Captured)> {
    todo!()
}
