//! A small pool of read-only connections to a WAL database: dashboard reads run beside ingest
//! instead of queueing behind it. Connections are opened on demand, up to a maximum.

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OpenFlags};

use crate::error::DbResult;

pub(crate) struct Pool {
    path: PathBuf,
    max: usize,
    state: Mutex<State>,
    returned: Condvar,
}

struct State {
    idle: Vec<Connection>,
    /// Connections handed out or idle.
    open: usize,
}

/// A pooled connection, returned to the pool when dropped (also on panic).
pub(crate) struct Lease<'a> {
    pool: &'a Pool,
    conn: Option<Connection>,
}

impl Pool {
    pub(super) fn new(path: PathBuf, max: usize) -> Self {
        Pool { path, max: max.max(1), state: Mutex::new(State { idle: Vec::new(), open: 0 }), returned: Condvar::new() }
    }

    /// Run `f` on a read connection, inside one read transaction so all its queries see the same
    /// snapshot.
    pub(super) fn read<T>(&self, f: impl FnOnce(&Connection) -> DbResult<T>) -> DbResult<T> {
        let conn = self.take()?;
        conn.execute_batch("begin")?;
        let result = f(&conn);
        // A failed end leaves the transaction open; `Lease::drop` then discards the connection.
        if let Err(e) = conn.execute_batch("commit") {
            tracing::warn!(error = %e, "could not end a read transaction");
        }
        result
    }

    fn take(&self) -> DbResult<Lease<'_>> {
        let mut state = self.lock();
        loop {
            if let Some(conn) = state.idle.pop() {
                return Ok(Lease { pool: self, conn: Some(conn) });
            }
            if state.open < self.max {
                state.open += 1;
                drop(state);
                return match self.open() {
                    Ok(conn) => Ok(Lease { pool: self, conn: Some(conn) }),
                    Err(e) => {
                        self.forget_one();
                        Err(e)
                    }
                };
            }
            state = self.returned.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn open(&self) -> DbResult<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
        let conn = Connection::open_with_flags(&self.path, flags)?;
        conn.busy_timeout(super::BUSY_TIMEOUT)?;
        Ok(conn)
    }

    fn give_back(&self, conn: Connection) {
        let mut state = self.lock();
        if conn.is_autocommit() {
            state.idle.push(conn);
        } else {
            state.open = state.open.saturating_sub(1);
        }
        drop(state);
        self.returned.notify_one();
    }

    fn forget_one(&self) {
        let mut state = self.lock();
        state.open = state.open.saturating_sub(1);
        drop(state);
        self.returned.notify_one();
    }

    /// Only pushes, pops and counter updates happen under the lock (none can panic halfway), so a
    /// poisoned lock still guards a consistent state.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Deref for Lease<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        // `conn` is only taken in `drop`, so it is always set while the lease is alive.
        self.conn.as_ref().unwrap_or_else(|| unreachable!("a lease holds its connection until dropped"))
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        match self.conn.take() {
            Some(conn) => self.pool.give_back(conn),
            None => self.pool.forget_one(),
        }
    }
}
