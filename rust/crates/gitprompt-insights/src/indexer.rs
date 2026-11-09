//! Keeps the prompt index (gitprompt-store `index`) in step with the raw rows, using the same
//! derivations the prompt API returns ([`crate::derivations::derive`]), so a value is computed in
//! one place only (ADR-0012).
//!
//! * [`Indexer::ensure`]: before every read. Rebuild if the rules version changed, else index new
//!   prompts and the turns ingest marked as changed (a Stop or a model call changes a prompt), and
//!   the telemetry metrics and slash commands that arrived since the last read.
//! * [`Indexer::reindex`]: after an edit of some prompts (hiding a secret).
//! * [`Indexer::rebuild`]: everything, e.g. after a remap, a compaction or `gitprompt reindex`.
//!
//! The rules version hashes what the values depend on besides the raw rows: [`CODE_VERSION`], each
//! spec's `origin` and `surface` rules, `injected` tags, `tool_kinds`, `tool_files` and `usage`, and
//! the timezone's name (day and hour buckets). Changing any of them rebuilds the index on next use,
//! with no migration.

use std::sync::Arc;

use gitprompt_core::Result;
use gitprompt_core::spec::Spec;
use gitprompt_store::Outcome;
use gitprompt_store::Store;
use gitprompt_store::index::Entry;

use crate::Specs;
use crate::derivations::Derived;
use crate::facts::Facts;
use crate::zone::Zone;

/// Prompts derived per batch during a (re)index.
pub const BATCH: usize = 500;

/// Bump whenever a derivation, a projection in [`entry`] or the index schema changes what is
/// stored: every index is then rebuilt on next use. (Python hashed its source files; a Rust binary
/// has none to hash.)
pub const CODE_VERSION: &str = "rust-1";

/// 16 hex characters of a SHA-256 over `code_version`, the zone's name and the rules of every spec
/// (in id order): `origin`, `surface`, `injected`, `tool_kinds`, `tool_files` (pattern sources) and
/// `usage`. Other spec data (names, mappings, docs) does not change it.
pub fn rules_version(_specs: &Specs, _zone: &dyn Zone, _code_version: &str) -> String {
    todo!()
}

/// Equal for texts that differ only in case and whitespace: 16 hex characters of a SHA-256 of the
/// lower-cased words joined by single spaces. `None` for no text or only whitespace.
pub fn text_hash(_typed: Option<&str>) -> Option<String> {
    todo!()
}

/// A slash command with exactly one leading slash, whichever event named it; `None` stays `None`
/// (and so does `""`).
pub fn slash(_command: Option<&str>) -> Option<String> {
    todo!()
}

/// The index row of a prompt, projected from [`crate::derivations::derive`]:
///
/// * `repo`: `context.git_root`, else `fields.cwd`; `repo_name`/`branch`: `context.git_repo` /
///   `git_branch`; `session`/`title`/`prompt_id`/`permission_mode`: mapped fields
/// * `day`/`hour`/`weekday`: `received_at` in `zone`
/// * `chars`, `code_blocks`, `urls`, `mentions`, `text_hash`: of the *typed* text (injected
///   context excluded); `secrets`: of the prompt as received
/// * `slash_command`: the turn's command (telemetry, expansion), else the typed one, via [`slash`]
/// * usage, times, counts: from the summary; `tool_calls` and `subagents` summed over names;
///   `files_written` and `denials` counted; `answered`: the summary has a response
/// * `model`: [`crate::facts::main_model`]; `tool_version`: `fields.tool_version`, else
///   `context.tool_version`
/// * `models`: usage per model; without model calls, the hook's `fields.model` with zero usage
/// * `tools`: (tool, kind, calls) per tool name; `work`: usage per query source with its kind
pub fn entry(_derived: &Derived, _zone: &dyn Zone) -> Result<Entry> {
    todo!()
}

/// A derived prompt's facts, from the same projection the index stores (models: every model of the
/// projection that has a name). Time buckets don't affect facts, so no zone is needed.
pub fn prompt_facts(_derived: &Derived, _spec: Option<&Spec>, _outcome: Option<Outcome>) -> Facts {
    todo!()
}

/// Computes index entries for a store. Holds no store, so one indexer can serve the daemon's store
/// and a desktop app's alike.
#[derive(Clone, Debug)]
pub struct Indexer {
    specs: Specs,
    zone: Arc<dyn Zone>,
    version: String,
}

impl Indexer {
    /// The machine's timezone ([`crate::zone::MachineZone`]) and [`CODE_VERSION`].
    pub fn new(_specs: Specs) -> Self {
        todo!()
    }

    /// An explicit timezone and code version (tests; a desktop app with its own timezone setting).
    pub fn with(_specs: Specs, _zone: Arc<dyn Zone>, _code_version: &str) -> Self {
        todo!()
    }

    /// [`rules_version`] of this indexer's specs, zone and code version.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn specs(&self) -> &Specs {
        &self.specs
    }

    pub fn zone(&self) -> &dyn Zone {
        self.zone.as_ref()
    }

    /// The entries of these prompts (unknown ids skipped), each derived with the events of its turn
    /// and its source's spec.
    pub fn entries(&self, _store: &Store, _ids: &[i64]) -> Result<Vec<Entry>> {
        todo!()
    }

    /// Recompute these prompts (duplicates counted once), in batches of [`BATCH`], in one
    /// transaction. Returns how many distinct ids were given.
    pub fn reindex(&self, _store: &Store, _ids: &[i64]) -> Result<usize> {
        todo!()
    }

    /// Drop the index and compute it again from the raw rows (telemetry included, so nothing is
    /// counted twice), stamped with this version, in one transaction. Returns the prompts indexed.
    pub fn rebuild(&self, _store: &Store) -> Result<usize> {
        todo!()
    }

    /// Bring the index up to date; cheap when nothing changed, so it runs before every read. A
    /// different stored version rebuilds; otherwise new telemetry is indexed, then the unindexed
    /// prompts and the prompts of marked turns are reindexed. Returns the prompts (re)indexed.
    pub fn ensure(&self, _store: &Store) -> Result<usize> {
        todo!()
    }
}
