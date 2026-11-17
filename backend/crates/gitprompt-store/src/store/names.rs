//! The names you gave things. Durable user data (`names` table), not part of the rebuildable index:
//! `gitprompt reindex` keeps every name. Only a session can be named today; a chapter will be.

use std::collections::BTreeMap;

use gitprompt_core::{Error, Result};

use super::Store;
use crate::decode;
use crate::sql::json_list;

/// What a name is given to. A closed list: a new kind is a variant here, nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    Session,
}

impl NameKind {
    /// The kind as the `names` table and the API's path spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            NameKind::Session => "session",
        }
    }

    /// The kind spelled `text`, if there is one.
    pub fn parse(text: &str) -> Option<NameKind> {
        [NameKind::Session].into_iter().find(|kind| kind.as_str() == text)
    }
}

/// Longest name, in characters: a note to yourself, not a paragraph.
pub const MAX_NAME_CHARS: usize = 80;
/// Longest thing named (a session id); a bound on what a client can stuff into the table.
pub const MAX_NAMED_ID_CHARS: usize = 256;

/// A name as stored: surrounding space cut, runs of white space made one space. Empty, over
/// [`MAX_NAME_CHARS`] or holding a control character is [`Error::Invalid`].
pub fn clean_name(raw: &str) -> Result<String> {
    if raw.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(Error::Invalid("name: no control characters".into()));
    }
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(Error::Invalid("name: not empty".into()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(Error::Invalid(format!("name: at most {MAX_NAME_CHARS} characters")));
    }
    Ok(name)
}

fn check_id(id: &str) -> Result<()> {
    if id.is_empty() || id.chars().count() > MAX_NAMED_ID_CHARS {
        return Err(Error::Invalid(format!("id: between 1 and {MAX_NAMED_ID_CHARS} characters")));
    }
    Ok(())
}

impl Store {
    /// Name `id` of `kind` (replacing its name); returns the name as stored (see [`clean_name`]).
    pub fn set_name(&self, kind: NameKind, id: &str, name: &str) -> Result<String> {
        let name = clean_name(name)?;
        check_id(id)?;
        self.db().write(|conn| {
            let upsert = "insert into names (kind, id, name) values (?1, ?2, ?3) \
                          on conflict (kind, id) do update set name = excluded.name";
            conn.prepare_cached(upsert)?.execute((kind.as_str(), id, &name))?;
            Ok(())
        })?;
        Ok(name)
    }

    /// Forget the name of `id` of `kind`; whether it had one.
    pub fn clear_name(&self, kind: NameKind, id: &str) -> Result<bool> {
        let rows = self
            .db()
            .write(|conn| Ok(conn.prepare_cached("delete from names where kind = ?1 and id = ?2")?.execute((kind.as_str(), id))?))?;
        Ok(rows > 0)
    }

    /// Every name given to a thing of `kind`, as `(id, name)`. The table only holds what you typed,
    /// so it stays small: find reads it whole to match names of any age.
    pub fn all_names(&self, kind: NameKind) -> Result<Vec<(String, String)>> {
        Ok(self.db().read(|conn| {
            let select = "select id, name from names where kind = ?1 order by id";
            decode::all(conn, select, (kind.as_str(),), |row| Ok((row.get::<_, String>("id")?, row.get::<_, String>("name")?)))
        })?)
    }

    /// The names given to these ids of `kind`, by id (ids without a name are absent).
    pub fn names_of(&self, kind: NameKind, ids: &[String]) -> Result<BTreeMap<String, String>> {
        let ids = json_list(ids)?;
        let rows = self.db().read(|conn| {
            let select = "select id, name from names where kind = ?1 and id in (select value from json_each(?2))";
            decode::all(conn, select, (kind.as_str(), ids), |row| Ok((row.get::<_, String>("id")?, row.get::<_, String>("name")?)))
        })?;
        Ok(rows.into_iter().collect())
    }
}
