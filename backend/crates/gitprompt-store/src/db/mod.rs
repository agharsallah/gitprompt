//! Connections. One writer connection behind a reentrant per-thread [`Gate`], plus (for a database
//! file) a [`Pool`] of read-only connections in WAL mode so reads never wait behind ingest.
//!
//! * [`Db::write`] runs a closure on the writer.
//! * [`Db::read`] runs it on a pooled reader, or on the writer when the current thread is inside
//!   the gate (its own uncommitted writes must be visible) or the database is in memory.
//! * [`Db::transaction`] keeps the gate for the whole closure, so every store call the closure
//!   makes on the same thread joins the transaction; other threads' writes wait for it.
//!
//! The writer's mutex is held only while one closure runs. Closures given to `write`/`read` only
//! run SQL on the connection they get; they never call back into the store (that would lock the
//! writer twice). Callbacks from callers (`rewrite`'s edit, `remap`'s mapper) run outside them.

mod files;
mod gate;
mod pool;

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{DbError, DbResult};
use crate::schema;
pub(crate) use files::make_private;
use gate::Gate;
use pool::Pool;

/// How long a connection waits for a lock before failing with `SQLITE_BUSY`.
pub(crate) const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Read connections of a database file.
const READERS: usize = 4;
/// The savepoint of a transaction opened inside another one.
const SAVEPOINT: &str = "gitprompt";

pub(crate) struct Db {
    gate: Gate,
    writer: Mutex<Connection>,
    readers: Option<Pool>,
}

impl Db {
    /// Open (creating it and its schema) a database file in WAL mode: directory 0700, file 0600.
    pub(crate) fn open(path: &Path) -> DbResult<Db> {
        files::prepare(path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let writer = Connection::open_with_flags(path, flags)?;
        writer.busy_timeout(BUSY_TIMEOUT)?;
        let mode: String = writer.query_row("pragma journal_mode = wal", [], |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            tracing::warn!(%mode, path = %path.display(), "the database is not in WAL mode: reads will wait behind writes");
        }
        schema::create(&writer)?;
        files::make_private(path)?;
        tracing::debug!(path = %path.display(), "opened the database");
        Ok(Db { gate: Gate::default(), writer: Mutex::new(writer), readers: Some(Pool::new(path.to_owned(), READERS)) })
    }

    /// A private in-memory database: one connection serves reads and writes.
    pub(crate) fn open_in_memory() -> DbResult<Db> {
        let writer = Connection::open_in_memory()?;
        schema::create(&writer)?;
        Ok(Db { gate: Gate::default(), writer: Mutex::new(writer), readers: None })
    }

    /// Run `f` on the writer connection.
    pub(crate) fn write<T>(&self, f: impl FnOnce(&Connection) -> DbResult<T>) -> DbResult<T> {
        let _pass = self.gate.enter();
        f(&self.lock_writer())
    }

    /// Run `f` on a connection that sees everything committed, and this thread's open transaction.
    pub(crate) fn read<T>(&self, f: impl FnOnce(&Connection) -> DbResult<T>) -> DbResult<T> {
        match &self.readers {
            Some(pool) if !self.gate.held_here() => pool.read(f),
            _ => self.write(f),
        }
    }

    /// Run `f` on the writer inside a transaction.
    pub(crate) fn atomic<T>(&self, f: impl FnOnce(&Connection) -> DbResult<T>) -> DbResult<T> {
        self.transaction(|| self.write(f))
    }

    /// Run `f` all-or-nothing: `Err` (or a panic) rolls back every write made on the writer by this
    /// thread meanwhile. Nested transactions become savepoints.
    pub(crate) fn transaction<T, E: From<DbError>>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        let _pass = self.gate.enter();
        let scope = self.write(Scope::begin)?;
        let mut open = OpenTransaction { db: self, scope, finished: false };
        let value = f()?;
        open.commit()?;
        Ok(value)
    }

    /// A panic while the writer was locked leaves SQLite itself consistent (an open transaction is
    /// rolled back by its `OpenTransaction`), so a poisoned lock still guards a usable connection.
    fn lock_writer(&self) -> MutexGuard<'_, Connection> {
        self.writer.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A transaction at the top level, or a savepoint inside one.
#[derive(Clone, Copy)]
enum Scope {
    Top,
    Nested,
}

impl Scope {
    fn begin(conn: &Connection) -> DbResult<Scope> {
        if conn.is_autocommit() {
            conn.execute_batch("begin immediate")?;
            Ok(Scope::Top)
        } else {
            conn.execute_batch(&format!("savepoint {SAVEPOINT}"))?;
            Ok(Scope::Nested)
        }
    }

    fn commit_sql(self) -> String {
        match self {
            Scope::Top => "commit".into(),
            Scope::Nested => format!("release {SAVEPOINT}"),
        }
    }

    fn rollback_sql(self) -> String {
        match self {
            Scope::Top => "rollback".into(),
            Scope::Nested => format!("rollback to {SAVEPOINT}; release {SAVEPOINT}"),
        }
    }
}

/// Rolls back when dropped unless committed: on `Err`, on a failed commit, and on panic.
struct OpenTransaction<'a> {
    db: &'a Db,
    scope: Scope,
    finished: bool,
}

impl OpenTransaction<'_> {
    fn commit(&mut self) -> DbResult<()> {
        self.db.write(|conn| Ok(conn.execute_batch(&self.scope.commit_sql())?))?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for OpenTransaction<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let sql = self.scope.rollback_sql();
        if let Err(e) = self.db.write(|conn| Ok(conn.execute_batch(&sql)?)) {
            tracing::error!(error = ?e, "could not roll back a transaction");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(db: &Db) -> i64 {
        db.read(|c| Ok(c.query_row("select count(*) from annotation", [], |r| r.get(0))?)).unwrap()
    }

    fn insert(db: &Db, id: i64) -> DbResult<()> {
        db.write(|c| Ok(c.execute("insert into annotation (id, data) values (?, '{}')", [id]).map(|_| ())?))
    }

    #[test]
    fn a_nested_transaction_that_fails_rolls_back_only_its_own_writes() {
        let db = Db::open_in_memory().unwrap();

        db.transaction(|| {
            insert(&db, 1)?;
            let inner: DbResult<()> = db.transaction(|| {
                insert(&db, 2)?;
                Err(crate::error::failure("inner"))
            });
            assert!(inner.is_err());
            Ok::<_, DbError>(())
        })
        .unwrap();

        assert_eq!(count(&db), 1);
    }

    #[test]
    fn a_panic_inside_a_transaction_rolls_it_back_and_the_store_keeps_working() {
        let db = Db::open_in_memory().unwrap();

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: DbResult<()> = db.transaction(|| {
                insert(&db, 1)?;
                panic!("boom")
            });
        }));

        assert!(panicked.is_err());
        assert_eq!(count(&db), 0);
        insert(&db, 2).unwrap();
        assert_eq!(count(&db), 1);
    }
}
