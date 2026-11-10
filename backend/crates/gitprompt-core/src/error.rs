//! Domain errors. Raised by the core crates and turned into HTTP responses (or Tauri command errors)
//! in one place, so nothing below the adapters knows about HTTP.

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A request that doesn't make sense (400).
    #[error("{0}")]
    Invalid(String),
    /// Something named doesn't exist (404).
    #[error("{0}")]
    NotFound(String),
    /// Valid, but the state on this machine doesn't allow it (409).
    #[error("{0}")]
    Conflict(String),
    /// A tool's config file can't be safely edited: invalid JSON, not an object... (409).
    #[error("{0}")]
    Config(String),
    /// Organization or user settings turn this source off. Reported, never worked around (409).
    #[error("{0}")]
    PolicyBlocked(String),
    /// A preference is set by an environment variable, which wins over the file (409).
    #[error("{0}")]
    Locked(String),
    /// A preference value (request, file or env var) doesn't fit (422).
    #[error("{0}")]
    InvalidPreferences(String),
    /// A source spec (bundled or user override) can't be loaded (500).
    #[error("{0}")]
    Spec(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Storage failure (500); the message only, so the core stays free of SQL types.
    #[error("storage: {0}")]
    Storage(String),
}

impl Error {
    /// The HTTP status the API answers this error with.
    pub fn status(&self) -> u16 {
        match self {
            Error::Invalid(_) => 400,
            Error::NotFound(_) => 404,
            Error::Conflict(_) | Error::Config(_) | Error::PolicyBlocked(_) | Error::Locked(_) => 409,
            Error::InvalidPreferences(_) => 422,
            Error::Spec(_) | Error::Io(_) | Error::Json(_) | Error::Storage(_) => 500,
        }
    }
}
