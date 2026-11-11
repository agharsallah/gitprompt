//! Housekeeping primitives: event usage and deletion, size, VACUUM, online backup.

use std::path::Path;

use rusqlite::MAIN_DB;
use rusqlite::backup::Progress;

use gitprompt_core::Result;

use super::{EventUsage, Store};
use crate::db;
use crate::decode::{self, count};

impl Store {
    /// Stored events per (source, event), largest first; `before`: only rows received earlier.
    ///
    /// Without `before` this reads the trigger-maintained `event_stats` (a few rows); with it, it
    /// scans the events older than `before`.
    pub fn event_usage(&self, before: Option<&str>) -> Result<Vec<EventUsage>> {
        Ok(self.db().read(|conn| {
            let select = match before {
                None => "select source, event, rows, bytes from event_stats where ?1 is null order by bytes desc, source, event",
                Some(_) => {
                    "select source, event, count(*) rows, sum(length(cast(fields as blob)) + length(cast(raw as blob))) bytes \
                     from event where received_at < ?1 group by source, event order by bytes desc, source, event"
                }
            };
            decode::all(conn, select, [before], |row| {
                Ok(EventUsage {
                    source: row.get("source")?,
                    event: decode::text_at(row, "event")?,
                    rows: count(row, "rows")?,
                    bytes: count(row, "bytes")?,
                })
            })
        })?)
    }

    /// Delete stored events of one kind (`event` `None` = events without a name), optionally only
    /// those received before `before`. Prompts and annotations are never touched.
    pub fn delete_events(&self, source: &str, event: Option<&str>, before: Option<&str>) -> Result<u64> {
        let deleted = self.db().write(|conn| {
            let mut delete =
                conn.prepare_cached("delete from event where source = ? and event is ? and received_at < coalesce(?, '9999')")?;
            Ok(delete.execute((source, event, before))?)
        })?;
        Ok(deleted as u64)
    }

    /// Recompute the per-(source, event) summary behind `event_usage` and `source_stats` from the
    /// stored events. Triggers keep it exact; this is for repair (`gitprompt reindex`) and after a
    /// remap renamed events.
    pub fn rebuild_event_stats(&self) -> Result<()> {
        Ok(self.db().write(|conn| Ok(conn.execute_batch(crate::schema::REBUILD_EVENT_STATS)?))?)
    }

    /// Bytes the database occupies (`page_count` × `page_size`, free pages included).
    pub fn size(&self) -> Result<u64> {
        let bytes: i64 = self.db().read(|conn| {
            Ok(conn.query_row("select page_count * page_size from pragma_page_count, pragma_page_size", [], |row| row.get(0))?)
        })?;
        Ok(u64::try_from(bytes).unwrap_or(0))
    }

    /// Rewrite the file without free pages, then fold the WAL back in.
    pub fn vacuum(&self) -> Result<()> {
        Ok(self.db().write(|conn| {
            conn.execute_batch("vacuum")?;
            conn.query_row("pragma wal_checkpoint(truncate)", [], |_| Ok(()))?;
            Ok(())
        })?)
    }

    /// Copy the database to `dest` (online backup API; consistent while ingest continues). The copy
    /// is private (mode 0600) like the database. New.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        self.db().read(|conn| Ok(conn.backup(MAIN_DB, dest, None::<fn(Progress)>)?))?;
        Ok(db::make_private(dest)?)
    }
}
