//! A running GitPrompt for the CLI (`gitprompt serve`) and the desktop shell: one per data
//! directory, enforced by an OS lock on `settings.lock_file()` (released when the process dies,
//! so a stale file left by a crash never blocks a start).
//!
//! [`Daemon::start`] returns once it is ready: lock taken, index ensured, spooled hook payloads
//! drained, first retention pass done, listener accepting. Retention then repeats every
//! [`DaemonOptions::retention_every`].

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use gitprompt_store::housekeeping::RETENTION_EVERY;

use gitprompt_core::Settings;

use crate::app::App;
use crate::error::{ApiError, ApiResult};
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
        DaemonOptions { addr: None, retention_every: RETENTION_EVERY }
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
    addr: SocketAddr,
    service: Service,
    /// Cancelling it stops the server (gracefully) and the background tasks.
    stop: CancellationToken,
    server: JoinHandle<ApiResult<()>>,
    retention: JoinHandle<()>,
    /// Held (OS lock) while the daemon runs.
    lock: File,
}

impl Daemon {
    pub async fn start(settings: Settings, app: AppOptions, options: DaemonOptions) -> Result<Daemon, DaemonError> {
        let lock = tokio::task::spawn_blocking({
            let settings = settings.clone();
            move || take_lock(&settings)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("lock task failed: {e}")))??;
        let addr = options.addr.unwrap_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, settings.port)));
        let app = tokio::task::spawn_blocking(move || App::new(settings, app))
            .await
            .map_err(|e| ApiError::Internal(format!("start task failed: {e}")))??;
        let service = app.service();
        tracing::info!(home = %service.settings().home.display(), "GitPrompt is starting");
        service.ensure_index().await?;
        match service.drain_spool().await {
            Ok(delivered) => tracing::info!(delivered, "drained the hook spool"),
            Err(e) => tracing::warn!(error = %e, "could not drain the hook spool"),
        }
        retention(&service).await;
        let listener = TcpListener::bind(addr).await.map_err(|source| DaemonError::Bind { addr, source })?;
        let addr = listener.local_addr()?;
        let stop = CancellationToken::new();
        let server = tokio::spawn(app.serve(listener, stop.clone().cancelled_owned()));
        let retention = tokio::spawn(repeat_retention(service.clone(), options.retention_every, stop.clone()));
        log_running(&service, addr).await;
        Ok(Daemon { addr, service, stop, server, retention, lock })
    }

    /// The bound address (the real port when started on port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The same facade the HTTP routes use (Tauri commands call it).
    pub fn service(&self) -> Service {
        self.service.clone()
    }

    /// Stop accepting, let in-flight requests finish, stop background tasks, close the store and
    /// release the lock. Returns when all of that is done.
    pub async fn shutdown(self) -> Result<(), DaemonError> {
        tracing::info!(addr = %self.addr, "GitPrompt is shutting down");
        self.stop.cancel();
        let joined = |e: tokio::task::JoinError| ApiError::Internal(format!("background task failed: {e}"));
        self.retention.await.map_err(joined)?;
        let served = self.server.await.map_err(joined)?;
        drop(self.service); // the store closes with its last handle
        self.lock.unlock()?;
        tracing::info!("GitPrompt stopped");
        served.map_err(Into::into)
    }
}

/// Take the data directory's lock (created with the home if missing); the pid is written for
/// whoever looks. `AlreadyRunning` when another process holds it.
fn take_lock(settings: &Settings) -> Result<File, DaemonError> {
    settings.token().map_err(ApiError::from)?; // creates the home owner-only, as the app would
    let lock_file = settings.lock_file();
    let mut file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&lock_file)?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Err(DaemonError::AlreadyRunning { lock_file }),
        Err(TryLockError::Error(e)) => return Err(e.into()),
    }
    file.set_len(0)?;
    write!(file, "{}", std::process::id())?;
    Ok(file)
}

/// The start-up summary: where it listens, the data it serves, the dashboard source.
async fn log_running(service: &Service, addr: SocketAddr) {
    match service.status().await {
        Ok(status) => tracing::info!(
            %addr,
            home = %status.home,
            db_bytes = status.db_bytes,
            prompts = status.prompts,
            events = status.events,
            dashboard = %status.dashboard.describe(),
            spool_left = status.spool.iter().map(|s| s.payloads).sum::<u64>(),
            "GitPrompt is running"
        ),
        Err(e) => tracing::info!(%addr, dashboard = %service.dashboard_info().describe(), status_error = %e, "GitPrompt is running"),
    }
}

async fn retention(service: &Service) {
    match service.retention_pass().await {
        Ok(removed) => tracing::info!(removed, "retention pass: deleted expired events"),
        Err(e) => tracing::warn!(error = %e, "retention pass failed"),
    }
}

/// A retention pass every `every` (the first one ran at start) until `stop`.
async fn repeat_retention(service: Service, every: Duration, stop: CancellationToken) {
    loop {
        tokio::select! {
            () = stop.cancelled() => return,
            () = tokio::time::sleep(every) => retention(&service).await,
        }
    }
}
