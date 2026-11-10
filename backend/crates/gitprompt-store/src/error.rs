//! Errors inside the crate. SQL errors stay typed (`DbError::Sql`) until they leave the crate as
//! `Error::Storage`; every other error passes through unchanged.

use gitprompt_core::Error;

#[derive(Debug)]
pub(crate) enum DbError {
    Sql(rusqlite::Error),
    Core(Error),
}

pub(crate) type DbResult<T> = std::result::Result<T, DbError>;

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sql(e)
    }
}

impl From<Error> for DbError {
    fn from(e: Error) -> Self {
        DbError::Core(e)
    }
}

/// JSON (de)serialization inside the store only fails on stored data that is not what the store
/// wrote: a storage failure, not a bad request.
impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        failure(format!("stored JSON: {e}"))
    }
}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        DbError::Core(Error::Io(e))
    }
}

impl From<DbError> for Error {
    fn from(e: DbError) -> Self {
        match e {
            DbError::Sql(e) => storage(e),
            DbError::Core(e) => e,
        }
    }
}

/// A SQLite error as the rest of GitPrompt sees it (core stays free of SQL types).
pub(crate) fn storage(e: rusqlite::Error) -> Error {
    Error::Storage(e.to_string())
}

/// A storage failure that is not a SQLite error (stored data the store cannot read back, a missing
/// SQLite feature).
pub(crate) fn failure(message: impl Into<String>) -> DbError {
    DbError::Core(Error::Storage(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_errors_leave_the_crate_as_storage_errors() {
        let e: Error = DbError::from(rusqlite::Error::QueryReturnedNoRows).into();
        assert!(matches!(e, Error::Storage(m) if m.contains("no rows")));
    }

    #[test]
    fn core_errors_pass_through_unchanged() {
        let e: Error = DbError::from(Error::Invalid("x".into())).into();
        assert!(matches!(e, Error::Invalid(m) if m == "x"));
    }
}
