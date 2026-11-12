//! The Connectors page: each connector's facts and activity, its spec, its recent captures, and
//! connecting, enabling and remapping a source.

use std::collections::{BTreeMap, HashMap};

use gitprompt_capture::connection::{self, StateFacts};
use gitprompt_capture::probe;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::{Spec, SpecFile, SpecSet, read_spec_file};
use gitprompt_insights::captures;
use gitprompt_store::Store;
use gitprompt_store::store::SourceStats;

use super::{Inner, Service};
use crate::error::{ApiError, ApiResult};
use crate::model::{CapturedSurface, Change, Enabled, HOURS, RecentCapture, SourceOut, SpecProblemOut, SpecText, Updated};
use crate::options::ToolVersionProbe;
use crate::query::iso;

/// What every connector's row is built from, read once per page.
struct SourcesPage<'a> {
    inner: &'a Inner,
    set: &'a SpecSet,
    stats: BTreeMap<String, SourceStats>,
    activity: HashMap<String, Vec<u64>>,
    preferences: Preferences,
}

impl SourcesPage<'_> {
    /// One connector's row, given its installed `version`.
    fn source(&self, spec: &Spec, version: Option<String>) -> ApiResult<SourceOut> {
        let connection = self.inner.connection(spec);
        let stats = self.stats.get(&spec.id);
        let facts = StateFacts::gather_for(&connection, &self.preferences, stats);
        let missing = probe::missing_capabilities(spec, version.as_deref());
        let problem = self.set.problems.iter().find(|p| p.fallback.as_deref() == Some(spec.id.as_str()));
        Ok(SourceOut {
            id: spec.id.clone(),
            name: spec.name.clone(),
            assistant: spec.assistant.clone(),
            part_of: spec.part_of.clone(),
            layer: spec.layer.clone(),
            summary: spec.summary.clone(),
            enabled: facts.enabled,
            detected: facts.detected,
            version,
            connected: facts.connected,
            outdated: facts.outdated,
            config_file: connection.config_file().display().to_string(),
            terms: spec.terms.clone(),
            after_connect: spec.after_connect.clone(),
            blocks: facts.blocks,
            warnings: facts.warnings,
            total: stats.map_or(0, |s| s.total),
            unmapped: stats.map_or(0, |s| s.unmapped),
            events: stats.map_or(0, |s| s.events),
            last_at: stats.and_then(|s| s.last_at.clone()),
            hourly: self.activity.get(&spec.id).cloned().unwrap_or_else(|| vec![0; HOURS]),
            collects: spec.documented.keys().filter(|e| *e != "*" && spec.keeps(Some(e))).cloned().collect(),
            not_stored: spec.drop_events.clone(),
            not_collected: spec.not_captured.clone(),
            capabilities: spec.capabilities.iter().filter(|c| !missing.contains_key(*c)).copied().collect(),
            needs_version: missing,
            join: spec.join,
            captures: captured_surfaces(spec),
            cannot_capture: spec.cannot_capture.clone(),
            spec: spec_file(self.set, spec)?.into(),
            spec_problem: problem.cloned().map(Into::into),
        })
    }
}

impl Service {
    pub async fn sources(&self) -> ApiResult<Vec<SourceOut>> {
        self.run(|inner| {
            let set = inner.spec_set();
            let page = SourcesPage {
                inner,
                set: &set,
                stats: inner.store.source_stats()?,
                activity: hourly(&inner.store, inner.now())?,
                preferences: inner.preferences.effective(),
            };
            let specs: Vec<&Spec> = set.specs.values().map(AsRef::as_ref).collect();
            let versions = tool_versions(inner.options.tool_version.as_ref(), &specs);
            specs.into_iter().zip(versions).map(|(spec, version)| page.source(spec, version)).collect()
        })
        .await
    }

    pub async fn recent_captures(&self) -> ApiResult<Vec<RecentCapture>> {
        self.run(|inner| {
            let (set, _) = inner.indexed()?;
            Ok(captures::recent(&inner.store, &set.specs)?.into_iter().map(Into::into).collect())
        })
        .await
    }

    pub async fn spec_problems(&self) -> ApiResult<Vec<SpecProblemOut>> {
        self.run(|inner| Ok(inner.spec_set().problems.iter().cloned().map(Into::into).collect())).await
    }

    pub async fn spec_text(&self, source: &str) -> ApiResult<SpecText> {
        let source = source.to_owned();
        self.run(move |inner| {
            let set = inner.spec_set();
            let file = spec_file(&set, Inner::spec(&set, &source)?.as_ref())?;
            let text = read_spec_file(&file)?;
            Ok(SpecText { file: file.into(), text })
        })
        .await
    }

    /// Refused (`Conflict`) while the source is disabled in GitPrompt.
    pub async fn connect(&self, source: &str, dry_run: bool) -> ApiResult<Change> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            connection::refuse_if_disabled(&spec, &inner.preferences.effective())?;
            Ok(inner.connection(&spec).connect(dry_run)?.into())
        })
        .await
    }

    pub async fn disconnect(&self, source: &str, dry_run: bool) -> ApiResult<Change> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            Ok(inner.connection(&spec).disconnect(dry_run)?.into())
        })
        .await
    }

    pub async fn enable(&self, source: &str) -> ApiResult<Enabled> {
        self.set_enabled(source, true).await
    }

    pub async fn disable(&self, source: &str) -> ApiResult<Enabled> {
        self.set_enabled(source, false).await
    }

    async fn set_enabled(&self, source: &str, enabled: bool) -> ApiResult<Enabled> {
        let source = source.to_owned();
        self.run(move |inner| {
            let spec = Inner::spec(&inner.spec_set(), &source)?;
            Ok(Enabled { enabled: connection::set_enabled(&spec, &inner.preferences, enabled)? })
        })
        .await
    }

    /// Rebuild mapped fields from stored raw payloads (and the session join), then the index.
    pub async fn remap(&self, source: &str) -> ApiResult<Updated> {
        let source = source.to_owned();
        self.run(move |inner| {
            let (set, indexer) = inner.indexed()?;
            let spec = Inner::spec(&set, &source)?;
            let updated = connection::remap(&spec, &inner.store)?;
            indexer.rebuild(&inner.store)?;
            Ok(Updated { updated })
        })
        .await
    }
}

/// Every tool's installed version, in `specs` order. A `--version` may take a while, so every
/// tool is asked at once.
fn tool_versions(probe: &dyn ToolVersionProbe, specs: &[&Spec]) -> Vec<Option<String>> {
    std::thread::scope(|scope| {
        #[expect(clippy::needless_collect, reason = "spawn every probe before joining any: collecting is what runs them in parallel")]
        let running: Vec<_> = specs.iter().map(|spec| scope.spawn(move || probe.version(spec))).collect();
        running.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    })
}

/// The file `spec` was loaded from; every loaded spec has one.
fn spec_file(set: &SpecSet, spec: &Spec) -> ApiResult<SpecFile> {
    set.files.get(&spec.id).cloned().ok_or_else(|| ApiError::Internal(format!("no spec file for {}", spec.id)))
}

/// The apps whose prompts reach `spec`'s connector, with what of each is captured.
fn captured_surfaces(spec: &Spec) -> Vec<CapturedSurface> {
    let surface = |(id, note): (&String, &String)| {
        let info = spec.surface_info(Some(id))?;
        Some(CapturedSurface { id: info.id, name: info.name, icon: info.icon, note: note.clone() })
    };
    spec.captures.iter().filter_map(surface).collect()
}

/// [`HOURS`] as an `i64` (24 always fits).
#[expect(clippy::cast_possible_wrap, reason = "24 fits any integer type")]
const HOURS_I64: i64 = HOURS as i64;

/// Prompts plus events per UTC hour of the last [`HOURS`] hours, per source, the current hour last.
fn hourly(store: &Store, now: jiff::Timestamp) -> ApiResult<HashMap<String, Vec<u64>>> {
    let hour = now.as_second().div_euclid(3600) * 3600;
    let start = jiff::Timestamp::from_second(hour - (HOURS_I64 - 1) * 3600).map_err(|e| ApiError::Internal(e.to_string()))?;
    let slots: HashMap<String, usize> = (0..HOURS)
        .filter_map(|i| {
            Some((start.checked_add(jiff::SignedDuration::from_hours(i64::try_from(i).ok()?)).ok()?.strftime("%Y-%m-%dT%H").to_string(), i))
        })
        .collect();
    let mut out: HashMap<String, Vec<u64>> = HashMap::new();
    for count in store.hourly(&iso(start))? {
        if let Some(&slot) = slots.get(&count.hour)
            && let Some(total) = out.entry(count.source).or_insert_with(|| vec![0; HOURS]).get_mut(slot)
        {
            *total += count.rows;
        }
    }
    Ok(out)
}
