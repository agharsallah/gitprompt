//! The weekly recaps already handled. Durable (`recaps` table), not part of the rebuildable index,
//! so a restart (or a `gitprompt reindex`) never sends a week's recap twice. The store keeps the
//! notification as JSON text: what it says is decided above the store.

use jiff::Timestamp;

use gitprompt_core::{Error, Result};

use super::Store;
use crate::decode;
use crate::time;

/// One handled recap: the scheduled instant whose week it summed up, and what was sent (`None`
/// when the week held nothing to say).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecapRow {
    pub due: Timestamp,
    pub notification: Option<String>,
}

impl Store {
    /// Remember that the recap due at `due` was handled, with what was sent (replacing any record
    /// of the same instant).
    pub fn record_recap(&self, due: Timestamp, notification: Option<&str>) -> Result<()> {
        self.db().write(|conn| {
            let upsert = "insert into recaps (due, notification) values (?1, ?2) \
                          on conflict (due) do update set notification = excluded.notification";
            conn.prepare_cached(upsert)?.execute((time::iso(due), notification))?;
            Ok(())
        })?;
        Ok(())
    }

    /// The latest recap handled, if any.
    pub fn latest_recap(&self) -> Result<Option<RecapRow>> {
        let row = self.db().read(|conn| {
            let select = "select due, notification from recaps order by due desc limit 1";
            decode::first(conn, select, (), |row| Ok((row.get::<_, String>("due")?, row.get::<_, Option<String>>("notification")?)))
        })?;
        row.map(|(due, notification)| {
            let due = due.parse().map_err(|_| Error::Invalid(format!("recaps: not a time: {due:?}")))?;
            Ok(RecapRow { due, notification })
        })
        .transpose()
    }
}
