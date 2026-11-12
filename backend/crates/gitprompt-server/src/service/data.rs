//! The Data page and housekeeping: where the data lives and how big it is, config-file backups,
//! compacting, deleting events, and the daemon's retention pass.

use std::path::{Path, PathBuf};

use gitprompt_core::Error;
use gitprompt_store::housekeeping;
use gitprompt_store::store::EventUsage;

use super::Service;
use crate::error::ApiResult;
use crate::model::{Backup, CompactIn, DataOut, DeleteIn, ReportOut, SourceEvents};
use crate::query::iso;

impl Service {
    pub async fn data(&self) -> ApiResult<DataOut> {
        self.run(|inner| {
            let settings = &inner.settings;
            Ok(DataOut {
                home: settings.home.display().to_string(),
                db_path: settings.db_path().display().to_string(),
                db_bytes: inner.store.size()?,
                receiver: settings.receiver(),
                events: events_by_source(inner.store.event_usage(None)?),
                backups: backups(&settings.backups()),
            })
        })
        .await
    }

    pub async fn compact(&self, body: CompactIn, dry_run: bool) -> ApiResult<ReportOut> {
        if body.older_than_days.is_some_and(|days| days < 1) {
            return Err(Error::Invalid("older_than_days must be at least 1".into()).into());
        }
        self.run(move |inner| {
            let set = inner.spec_set();
            Ok(housekeeping::compact(&inner.store, &set.specs, body.older_than_days, inner.now(), !dry_run)?.into())
        })
        .await
    }

    pub async fn delete_events(&self, body: DeleteIn, dry_run: bool) -> ApiResult<ReportOut> {
        if body.source.is_none() && body.before.is_none() {
            return Err(Error::Invalid("choose a source or a date: deleting every event is not offered".into()).into());
        }
        let before = body.before.as_deref().map(|b| crate::query::instant("before", b)).transpose()?;
        self.run(move |inner| {
            let before = before.unwrap_or_else(|| iso(inner.now()));
            let removals = housekeeping::selected(&inner.store, body.source.as_deref(), Some(&before))?;
            Ok(housekeeping::plan(&inner.store, removals, !dry_run)?.into())
        })
        .await
    }

    /// One retention pass with the `event_retention_days` preference at the app clock's now.
    /// Returns the events deleted (0 when retention is off).
    pub async fn retention_pass(&self) -> ApiResult<u64> {
        self.run(|inner| Ok(housekeeping::retention_pass(&inner.store, &inner.preferences, inner.now())?)).await
    }
}

/// Stored raw events summed per source (the store reports them per source and event), largest
/// first.
fn events_by_source(usage: Vec<EventUsage>) -> Vec<SourceEvents> {
    let mut events: Vec<SourceEvents> = Vec::new();
    for usage in usage {
        let Some(source) = events.iter_mut().find(|s| s.source == usage.source) else {
            events.push(SourceEvents { source: usage.source, rows: usage.rows, bytes: usage.bytes });
            continue;
        };
        source.rows += usage.rows;
        source.bytes += usage.bytes;
    }
    events.sort_by_key(|s| std::cmp::Reverse(s.bytes));
    events
}

/// A file's modification time, ISO-8601 UTC.
pub(super) fn modified(meta: &std::fs::Metadata) -> String {
    meta.modified().ok().and_then(|t| jiff::Timestamp::try_from(t).ok()).map(iso).unwrap_or_default()
}

/// Config-file copies in `folder`, newest first.
fn backups(folder: &Path) -> Vec<Backup> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut files: Vec<(PathBuf, std::fs::Metadata)> =
        entries.filter_map(Result::ok).filter_map(|e| Some((e.path(), e.metadata().ok()?))).filter(|(_, m)| m.is_file()).collect();
    files.sort_by_key(|(_, m)| std::cmp::Reverse(m.modified().ok()));
    files
        .into_iter()
        .map(|(path, meta)| Backup {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.display().to_string(),
            bytes: meta.len(),
            modified: modified(&meta),
        })
        .collect()
}
