//! A running GitPrompt for the CLI (`gitprompt serve`) and the desktop shell: one per data
//! directory, enforced by an OS lock on `settings.lock_file()` (released when the process dies,
//! so a stale file left by a crash never blocks a start).
//!
//! [`Daemon::start`] returns once it is ready: lock taken, index ensured, spooled hook payloads
//! drained, first retention pass done, listener accepting. Retention then repeats every
//! [`DaemonOptions::retention_every`].

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use gitprompt_core::Settings;

use crate::error::ApiError;
use crate::options::AppOptions;
use crate::service::Service;

#[derive(Clone, Debug)]
pub struct DaemonOptions {
    /// Where to listen. Default `127.0.0.1:<settings.port>`; tests use port 0.
    pub addr: Option<SocketAddr>,
    /// Default `gitprompt_store::housekeeping::RETENTION_EVERY` (6 h).
    pub retention_every: Duration,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    /// Another daemon (CLI or desktop) holds the lock of this data directory.
    #[error("GitPrompt is already running for this data directory (lock held on {})", lock_file.display())]
    AlreadyRunning { lock_file: PathBuf },
    /// The address can't be bound (port in use...).
    #[error("cannot listen on {addr}: {source}")]
    Bind { addr: SocketAddr, source: std::io::Error },
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct Daemon {
    _private: (),
}

impl Daemon {
    pub async fn start(_settings: Settings, _app: AppOptions, _options: DaemonOptions) -> Result<Daemon, DaemonError> {
        todo!()
    }

    /// The bound address (the real port when started on port 0).
    pub fn local_addr(&self) -> SocketAddr {
        todo!()
    }

    /// The same facade the HTTP routes use (Tauri commands call it).
    pub fn service(&self) -> Service {
        todo!()
    }

    /// Stop accepting, let in-flight requests finish, stop background tasks, close the store and
    /// release the lock. Returns when all of that is done.
    pub async fn shutdown(self) -> Result<(), DaemonError> {
        todo!()
    }
}
