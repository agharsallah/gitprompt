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

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest, Sha256};

use gitprompt_core::spec::{OriginRule, Spec, SurfaceRule, ToolKind, Usage};
use gitprompt_core::{Json, JsonMap, Result};
use gitprompt_store::index::{Entry, IndexRow, ModelUsage, SessionCommand, SessionMetric, ToolCalls, UsageTotals, WorkUsage};
use gitprompt_store::{Annotation, EventRow, Outcome, PromptRow, PromptView, Store};

use crate::Specs;
use crate::derivations::{self, COMMAND_EVENT, Derived, METRIC_PREFIX, derive, prompt_stats, work_kind};
use crate::facts::{Facts, facts, main_model};
use crate::origin;
use crate::text::{text_of, truthy, words};
use crate::zone::{MachineZone, Zone, local_time};

/// Prompts derived per batch during a (re)index.
pub const BATCH: usize = 500;

/// Bump whenever a derivation, a projection in [`entry`] or the index schema changes what is
/// stored: every index is then rebuilt on next use. (Python hashed its source files; a Rust binary
/// has none to hash.)
pub const CODE_VERSION: &str = "rust-2";

/// 16 hex characters of a SHA-256 over `code_version`, the zone's name and the rules of every spec
/// (in id order): `origin`, `surface`, `injected`, `tool_kinds`, `tool_files` (pattern sources) and
/// `usage`. Other spec data (names, mappings, docs) does not change it.
pub fn rules_version(specs: &Specs, zone: &dyn Zone, code_version: &str) -> String {
    /// What the index depends on of one spec.
    #[derive(Serialize)]
    struct Rules<'s> {
        id: &'s str,
        origin: &'s [OriginRule],
        surface: &'s [SurfaceRule],
        injected: &'s [String],
        tool_kinds: &'s indexmap::IndexMap<String, ToolKind>,
        tool_files: Vec<(&'s str, &'s str)>,
        usage: &'s Usage,
    }
    // `Specs` is ordered by id.
    let rules: Vec<Rules<'_>> = specs
        .values()
        .map(|s| Rules {
            id: &s.id,
            origin: &s.origin,
            surface: &s.surface,
            injected: &s.injected,
            tool_kinds: &s.tool_kinds,
            tool_files: s.tool_files.iter().map(|(tool, pattern)| (tool.as_str(), pattern.as_str())).collect(),
            usage: &s.usage,
        })
        .collect();
    let data = (code_version, zone.name(), rules);
    // Strings, unit enums and maps with string keys always serialize; should that ever change, the
    // error text still gives a stable version.
    let bytes = serde_json::to_vec(&data).unwrap_or_else(|e| {
        tracing::error!(error = %e, "rules could not be serialized for the rules version");
        e.to_string().into_bytes()
    });
    hex16(&bytes)
}

/// The first 16 hex characters of the SHA-256 of `bytes`.
fn hex16(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Equal for texts that differ only in case and whitespace: 16 hex characters of a SHA-256 of the
/// lower-cased words joined by single spaces. `None` for no text or only whitespace.
pub fn text_hash(typed: Option<&str>) -> Option<String> {
    let lower = typed?.to_lowercase();
    let joined = words(&lower).collect::<Vec<_>>().join(" ");
    (!joined.is_empty()).then(|| hex16(joined.as_bytes()))
}

/// A slash command with exactly one leading slash, whichever event named it; `None` stays `None`
/// (and so does `""`).
pub fn slash(command: Option<&str>) -> Option<String> {
    command.filter(|c| !c.is_empty()).map(|c| format!("/{}", c.trim_start_matches('/')))
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
pub fn entry(derived: &Derived, zone: &dyn Zone) -> Result<Entry> {
    let local = local_time(&derived.prompt.row.received_at, zone)?;
    let mut entry = project(derived);
    entry.row.day = local.day;
    entry.row.hour = local.hour;
    entry.row.weekday = local.weekday;
    Ok(entry)
}

/// A mapped field (or context value) as text.
fn text(map: &JsonMap, key: &str) -> Option<String> {
    map.get(key).and_then(text_of)
}

/// A field (or context value) when it is truthy, as text.
fn truthy_text(map: &JsonMap, key: &str) -> Option<String> {
    map.get(key).filter(|v| truthy(v)).and_then(text_of)
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn whole(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn sum(values: impl Iterator<Item = u64>) -> i64 {
    whole(values.sum())
}

/// [`entry`] without the time buckets (left at their defaults), which need a zone.
fn project(derived: &Derived) -> Entry {
    let row = &derived.prompt.row;
    let (fields, context, stats, summary) = (&row.fields, &row.context, &derived.stats, &derived.summary);
    let typed = derived.classified.typed_text.as_deref();
    // What the user wrote is counted in the typed text; secrets in the prompt as received.
    let typed_stats = prompt_stats(typed.unwrap_or_default());
    let index_row = IndexRow {
        id: row.id,
        prompt_id: text(fields, "prompt_id"),
        source: row.source.clone(),
        session: text(fields, "session_id"),
        repo: truthy_text(context, "git_root").or_else(|| text(fields, "cwd")),
        repo_name: text(context, "git_repo"),
        branch: text(context, "git_branch"),
        title: text(fields, "session_title"),
        received_at: row.received_at.clone(),
        origin: origin::name(derived.origin()).to_owned(),
        typed_text: typed.map(String::from),
        chars: count(typed.unwrap_or_default().chars().count()),
        text_hash: text_hash(typed),
        slash_command: slash(summary.slash_command.as_deref().filter(|c| !c.is_empty()).or(stats.slash_command.as_deref())),
        code_blocks: count(typed_stats.code_blocks),
        urls: count(typed_stats.urls.len()),
        mentions: count(typed_stats.at_mentions.len()),
        secrets: count(stats.secrets.len()),
        usage: summary.usage.clone(),
        api_time_s: summary.api_time_s,
        api_errors: whole(summary.api_errors),
        response_s: summary.response_s,
        ended_by: summary.ended_by.clone(),
        tool_calls: sum(summary.tool_calls.values().copied()),
        tool_failures: whole(summary.tool_failures),
        tool_time_s: summary.tool_time_s,
        subagent_tool_calls: whole(summary.subagent_tool_calls),
        subagents: sum(summary.subagents.values().copied()),
        files_written: count(summary.files_written.len()),
        git_commits: whole(summary.git_commits),
        compactions: whole(summary.compactions),
        denials: count(summary.permission_denied.len()),
        rejections: whole(summary.tool_rejections),
        model: main_model(summary, fields),
        permission_mode: text(fields, "permission_mode"),
        tool_version: truthy_text(fields, "tool_version").or_else(|| text(context, "tool_version")),
        reasoning_tokens: summary.reasoning_tokens,
        answered: summary.response.as_ref().is_some_and(truthy),
        surface: derived.surface.clone(),
        ..IndexRow::default()
    };
    // Models with usage come from model calls; without them, the model the tool reported with the
    // prompt (hooks that carry one) still counts the prompt for that model, with no usage.
    let models = if summary.model_usage.is_empty() {
        truthy_text(fields, "model").map(|model| ModelUsage { model: Some(model), usage: UsageTotals::default() }).into_iter().collect()
    } else {
        summary.model_usage.iter().map(|(model, usage)| ModelUsage { model: model.clone(), usage: usage.clone() }).collect()
    };
    let tools = summary
        .tool_calls
        .iter()
        .map(|(tool, calls)| ToolCalls {
            tool: tool.clone(),
            kind: tool.as_ref().and_then(|t| summary.tool_kind_of.get(t)).copied().unwrap_or_default(),
            calls: whole(*calls),
        })
        .collect();
    let work = summary
        .work_usage
        .iter()
        .map(|(source, usage)| WorkUsage {
            work: work_kind(source.as_deref()).as_str().to_owned(),
            query_source: source.clone(),
            usage: usage.clone(),
        })
        .collect();
    Entry { row: index_row, models, tools, work }
}

/// A derived prompt's facts, from the same projection the index stores (models: every model of the
/// projection that has a name). Time buckets don't affect facts, so no zone is needed.
pub fn prompt_facts(derived: &Derived, spec: Option<&Spec>, outcome: Option<Outcome>) -> Facts {
    let projected = project(derived);
    let models = projected.models.iter().filter_map(|m| m.model.clone()).collect();
    facts(&projected.row, spec, outcome, Some(models))
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
    pub fn new(specs: Specs) -> Self {
        Self::with(specs, Arc::new(MachineZone), CODE_VERSION)
    }

    /// An explicit timezone and code version (tests; a desktop app with its own timezone setting).
    pub fn with(specs: Specs, zone: Arc<dyn Zone>, code_version: &str) -> Self {
        let version = rules_version(&specs, zone.as_ref(), code_version);
        Indexer { specs, zone, version }
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
    ///
    /// Two queries whatever the number of ids: the prompts, then the events of all their turns. A
    /// prompt whose `received_at` is not a timestamp is skipped with a warning rather than failing
    /// every read of the index.
    pub fn entries(&self, store: &Store, ids: &[i64]) -> Result<Vec<Entry>> {
        let rows = store.prompts(ids)?;
        let mut turns: Vec<String> = rows.iter().filter_map(turn_of).map(String::from).collect();
        turns.sort_unstable();
        turns.dedup();
        let mut events: HashMap<String, Vec<EventRow>> = HashMap::new();
        for event in store.events_for_prompts(&turns)? {
            if let Some(pid) = event.prompt_id.clone() {
                events.entry(pid).or_default().push(event);
            }
        }
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let turn = turn_of(&row).and_then(|pid| events.get(pid)).map_or(&[][..], Vec::as_slice);
            let spec = self.specs.get(&row.source).map(AsRef::as_ref);
            let derived = derive(&view(row), turn, spec);
            match entry(&derived, self.zone.as_ref()) {
                Ok(entry) => out.push(entry),
                Err(error) => tracing::warn!(id = derived.prompt.row.id, %error, "prompt left out of the index"),
            }
        }
        Ok(out)
    }

    /// Recompute these prompts (duplicates counted once), in batches of [`BATCH`], in one
    /// transaction. Returns how many distinct ids were given.
    pub fn reindex(&self, store: &Store, ids: &[i64]) -> Result<usize> {
        store.transaction(|store| self.reindex_in(store, ids.to_vec()))
    }

    /// Drop the index and compute it again from the raw rows (telemetry included, so nothing is
    /// counted twice), stamped with this version, in one transaction. Returns the prompts indexed.
    pub fn rebuild(&self, store: &Store) -> Result<usize> {
        store.transaction(|store| {
            let index = store.index();
            index.drop_all()?;
            index.create()?;
            index.set_version(&self.version)?;
            self.index_telemetry(store)?;
            self.reindex_in(store, store.prompt_ids()?)
        })
    }

    /// Bring the index up to date; cheap when nothing changed, so it runs before every read. A
    /// different stored version rebuilds; otherwise new telemetry is indexed, then the unindexed
    /// prompts and the prompts of marked turns are reindexed. Returns the prompts (re)indexed.
    pub fn ensure(&self, store: &Store) -> Result<usize> {
        if store.index().version()?.as_deref() != Some(self.version.as_str()) {
            tracing::info!(version = %self.version, "rules changed: rebuilding the prompt index");
            return self.rebuild(store);
        }
        store.transaction(|store| {
            self.index_telemetry(store)?;
            let index = store.index();
            let mut ids = index.unindexed()?;
            ids.extend(store.prompts_of_turns(&index.take_marked()?)?);
            self.reindex_in(store, ids)
        })
    }

    /// [`Self::reindex`] inside a transaction the caller holds.
    fn reindex_in(&self, store: &Store, mut ids: Vec<i64>) -> Result<usize> {
        ids.sort_unstable();
        ids.dedup();
        for batch in ids.chunks(BATCH) {
            store.index().upsert(&self.entries(store, batch)?)?;
        }
        Ok(ids.len())
    }

    /// Telemetry metrics and slash commands that arrived since the last read (by event id).
    fn index_telemetry(&self, store: &Store) -> Result<()> {
        let index = store.index();
        let events = store.events_after(index.telemetry_after()?, METRIC_PREFIX, &[COMMAND_EVENT])?;
        let Some(last) = events.last() else { return Ok(()) };
        let metrics: Vec<SessionMetric> = events
            .iter()
            .filter_map(|e| {
                let (name, value) = derivations::metric(e)?;
                Some(SessionMetric { event_id: e.id, session: e.session_id.clone(), received_at: e.received_at.clone(), name, value })
            })
            .collect();
        let commands: Vec<SessionCommand> = events
            .iter()
            .filter_map(|e| {
                let (command, source) = derivations::command(e)?;
                Some(SessionCommand { event_id: e.id, session: e.session_id.clone(), received_at: e.received_at.clone(), command, source })
            })
            .collect();
        index.add_telemetry(last.id, &metrics, &commands)
    }
}

/// The prompt id a stored prompt's turn events carry.
fn turn_of(row: &PromptRow) -> Option<&str> {
    row.fields.get("prompt_id").and_then(Json::as_str).filter(|pid| !pid.is_empty())
}

/// A stored row as the derivations take it. The index stores no position in the session (turn,
/// previous prompt), so the row needs none: Python derived index rows from bare rows too.
fn view(row: PromptRow) -> PromptView {
    PromptView { row, annotation: Annotation::default(), turn: 0, prev_at: None, next_head: None }
}
