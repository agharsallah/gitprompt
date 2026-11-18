//! Possible leaks as stored: the masked findings of the index (`prompt_index_leak`, rebuilt with
//! it) read through the prompt filters, and what you decided about them (`leak_state`, durable).

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use gitprompt_core::Result;

use crate::Store;
use crate::decode::{self, text_at};
use crate::error::DbResult;
use crate::filter::PromptFilter;
use crate::sql::filter::conditions;
use crate::sql::{Sql, int};

/// Findings read per request at most (newest first).
pub const LEAKS_READ: usize = 1000;

/// A finding of the index with the prompt it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredLeak {
    pub finding: String,
    pub value_key: String,
    pub kind: String,
    pub place: String,
    pub masked: String,
    /// The prompt (its row id) whose turn holds it, when it was received, and its session.
    pub prompt: i64,
    pub received_at: String,
    pub session: Option<String>,
}

/// A decision about one finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakState {
    pub finding: String,
    pub value_key: String,
    pub kind: String,
    pub masked: String,
    pub place: String,
    pub prompt: i64,
    /// `hidden` or `not_sensitive`.
    pub state: String,
    /// When it was decided (the app clock).
    pub at: String,
}

const COLUMNS: &str = "l.finding, l.value_key, l.kind, l.place, l.masked, l.id prompt, i.received_at, i.session";

impl Store {
    /// The findings of the prompts the filter keeps, newest first, at most [`LEAKS_READ`].
    pub fn leaks(&self, filter: &PromptFilter) -> Result<Vec<StoredLeak>> {
        let query = Sql::new(format!("select {COLUMNS} from prompt_index_leak l join prompt_index i on i.id = l.id where "))
            .push(conditions(filter, &self.running_since())?)
            .push(" order by i.id desc, l.rowid limit ")
            .bind(int(LEAKS_READ));
        Ok(self.db().read(|conn| decode::all(conn, query.text(), query.params(), stored))?)
    }

    /// One finding by its id, wherever it is.
    pub fn leak(&self, finding: &str) -> Result<Option<StoredLeak>> {
        let select = format!("select {COLUMNS} from prompt_index_leak l join prompt_index i on i.id = l.id where l.finding = ?1 limit 1");
        Ok(self.db().read(|conn| decode::first(conn, &select, [finding], stored))?)
    }

    /// Every decision, oldest first.
    pub fn leak_states(&self) -> Result<Vec<LeakState>> {
        let select = "select finding, value_key, kind, masked, place, prompt, state, at from leak_state order by at, finding";
        Ok(self.db().read(|conn| decode::all(conn, select, [], state))?)
    }

    /// Record a decision (replacing any earlier one about the same finding).
    pub fn set_leak_state(&self, state: &LeakState) -> Result<()> {
        let s = state;
        Ok(self.db().write(|conn| {
            conn.prepare_cached("insert or replace into leak_state (finding, value_key, kind, masked, place, prompt, state, at) values (?, ?, ?, ?, ?, ?, ?, ?)")?
                .execute((&s.finding, &s.value_key, &s.kind, &s.masked, &s.place, s.prompt, &s.state, &s.at))?;
            Ok(())
        })?)
    }

    /// Forget the decision about a finding; whether there was one.
    pub fn clear_leak_state(&self, finding: &str) -> Result<bool> {
        Ok(self.db().write(|conn| Ok(conn.prepare_cached("delete from leak_state where finding = ?")?.execute([finding])? > 0))?)
    }
}

fn stored(row: &Row<'_>) -> DbResult<StoredLeak> {
    Ok(StoredLeak {
        finding: row.get("finding")?,
        value_key: row.get("value_key")?,
        kind: row.get("kind")?,
        place: row.get("place")?,
        masked: row.get("masked")?,
        prompt: row.get("prompt")?,
        received_at: row.get("received_at")?,
        session: text_at(row, "session")?,
    })
}

fn state(row: &Row<'_>) -> DbResult<LeakState> {
    Ok(LeakState {
        finding: row.get("finding")?,
        value_key: row.get("value_key")?,
        kind: row.get("kind")?,
        masked: row.get("masked")?,
        place: row.get("place")?,
        prompt: row.get("prompt")?,
        state: row.get("state")?,
        at: row.get("at")?,
    })
}
