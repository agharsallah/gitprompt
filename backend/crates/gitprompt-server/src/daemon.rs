//! A running GitPrompt for the CLI (`gitprompt serve`) and the desktop shell: one per data
//! directory, enforced by an OS lock on `settings.lock_file()` (released when the process dies,
//! so a stale file left by a crash never blocks a start).
//!
//! [`Daemon::start`] returns once it answers: lock taken, index ensured, first retention pass done,
//! the spool's drain lock held, listener accepting. Then, in the background, the hook payloads
//! spooled while it was down are drained (a backlog of thousands takes seconds; the dashboard
//! shows what is already stored meanwhile; a `gitprompt spool drain` waits for it;
//! [`Daemon::spool_drained`]), and retention repeats every [`DaemonOptions::retention_every`],
//! health samples every [`DaemonOptions::health_every`]. From then on the notifier ([`crate::notify`])
//! checks what deserves a notification after each capture, and the weekly recap ([`crate::recap`])
//! waits for its day and hour.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use gitprompt_capture::spool::LockedSpool;
use gitprompt_store::housekeeping::RETENTION_EVERY;

use gitprompt_core::Settings;

use crate::app::App;
use crate::error::{ApiError, ApiResult};
use crate::health::{Limits, Monitor, ProcessSampler, SAMPLE_EVERY};
use crate::notify::{self, Notifier};
use crate::options::AppOptions;
use crate::recap;
use crate::service::Service;

#[derive(Clone, Debug)]
pub struct DaemonOptions {
    /// Where to listen. Default `127.0.0.1:<settings.port>`; tests use port 0.
    pub addr: Option<SocketAddr>,
    /// Default `gitprompt_store::housekeeping::RETENTION_EVERY` (6 h).
    pub retention_every: Duration,
    /// How often the process is sampled for [`crate::health`]. Default
    /// [`crate::health::SAMPLE_EVERY`] (30 s).
    pub health_every: Duration,
    /// When a health sample warns.
    pub health_limits: Limits,
    /// The longest wait of the weekly recap before it reads the clock again. Default
    /// [`crate::recap::CHECK_EVERY`] (15 min); tests on a moving clock set a short one.
    pub recap_every: Duration,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        DaemonOptions {
            addr: None,
            retention_every: RETENTION_EVERY,
            health_every: SAMPLE_EVERY,
            health_limits: Limits::default(),
            recap_every: recap::CHECK_EVERY,
        }
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

#[derive(Debug)]
pub struct Daemon {
    addr: SocketAddr,
    service: Service,
    /// Cancelling it stops the server (gracefully) and the background tasks.
    stop: CancellationToken,
    server: JoinHandle<ApiResult<()>>,
    /// The spool drain, then retention and health samples (see [`background`]).
    background: JoinHandle<()>,
    /// Cancelled once the start-up spool drain is over (done, failed or stopped).
    drained: CancellationToken,
    /// Held (OS lock) while the daemon runs.
    lock: File,
}

impl Daemon {
    pub async fn start(settings: Settings, app: AppOptions, options: DaemonOptions) -> Result<Daemon, DaemonError> {
        let lock = blocking("lock", {
            let settings = settings.clone();
            move || take_lock(&settings)
        })
        .await?;
        let addr = options.addr.unwrap_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, settings.port)));
        let app = blocking("start", move || App::new(settings, app).map_err(DaemonError::from)).await?;
        let service = app.service();
        prepare(&service).await?;
        let spool = lock_spool(&service).await;
        let listener = TcpListener::bind(addr).await.map_err(|source| DaemonError::Bind { addr, source })?;
        let addr = listener.local_addr()?;
        let stop = CancellationToken::new();
        let server = tokio::spawn(app.serve(listener, stop.clone().cancelled_owned()));
        log_running(&service, addr).await;
        let drained = CancellationToken::new();
        let background = tokio::spawn(background(service.clone(), options, stop.clone(), Drain { spool, drained: drained.clone() }));
        Ok(Daemon { addr, service, stop, server, background, drained, lock })
    }

    /// The bound address (the real port when started on port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Resolves once the hook payloads spooled before the start are drained (or the drain failed
    /// or was stopped): what the daemon serves from then on includes them.
    pub async fn spool_drained(&self) {
        self.drained.cancelled().await;
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
        self.background.await.map_err(joined)?;
        let served = self.server.await.map_err(joined)?;
        drop(self.service); // the store closes with its last handle
        self.lock.unlock()?;
        tracing::info!("GitPrompt stopped");
        served.map_err(Into::into)
    }
}

/// Run `f` on the blocking pool; `what` names the task when it panics.
async fn blocking<T: Send + 'static>(what: &str, f: impl FnOnce() -> Result<T, DaemonError> + Send + 'static) -> Result<T, DaemonError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| ApiError::Internal(format!("{what} task failed: {e}")))?
}

/// The work before listening: tool versions (in the background), the index, a first retention
/// pass. Quick on every start; the spool, which may not be, is drained after ([`background`]).
async fn prepare(service: &Service) -> Result<(), DaemonError> {
    let settings = service.settings();
    tracing::info!(home = %settings.home.display(), profile = %settings.profile, "GitPrompt is starting");
    service.warm_tool_versions();
    service.ensure_index().await?;
    retention(service).await;
    Ok(())
}

/// The spool's drain lock, taken before the daemon answers, so a `gitprompt spool drain` run once
/// it answers waits for the start-up drain instead of racing it. `None` (logged) when it cannot be
/// taken: the daemon still starts, and the spool waits for the next one.
async fn lock_spool(service: &Service) -> Option<LockedSpool> {
    service.lock_spool().await.inspect_err(|e| tracing::warn!(error = %e, "could not lock the hook spool; it is not drained")).ok()
}

/// The start-up drain: the spool's lock (if taken) and the token cancelled once it is over.
struct Drain {
    spool: Option<LockedSpool>,
    drained: CancellationToken,
}

/// The work after the listener is up, until `stop`: deliver the hook spool, start the notifier
/// (so what the spool held is never notified, and it listens before `drained` is cancelled), send
/// a weekly recap missed while the daemon was down (before `drained` too, so it counts what the
/// spool held), then retention and health samples on their schedules, notifications after each
/// capture and the next recap. The health rules' clock starts after the drain, busy by nature.
async fn background(service: Service, options: DaemonOptions, stop: CancellationToken, drain: Drain) {
    if let Some(spool) = drain.spool {
        drain_spool(&service, spool, &stop).await;
    }
    let notifier = Notifier::start(service.clone()).await;
    recap::send_due(&service).await;
    drain.drained.cancel();
    tokio::join!(
        repeat_retention(service.clone(), options.retention_every, stop.clone()),
        repeat_health(service.clone(), options.health_every, options.health_limits, stop.clone()),
        notify::run(notifier, stop.clone()),
        recap::run(service, options.recap_every, stop),
    );
}

/// Deliver the hook payloads spooled while the daemon was down, holding its lock until done; a
/// shutdown stops it between two payloads (the rest stays queued for the next start).
async fn drain_spool(service: &Service, spool: LockedSpool, stop: &CancellationToken) {
    let stopping = stop.clone();
    match service.drain_spool(spool, move || stopping.is_cancelled()).await {
        Ok(delivered) => tracing::info!(delivered, "drained the hook spool"),
        Err(e) => tracing::warn!(error = %e, "could not drain the hook spool"),
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

/// A health sample every `every` until `stop`. The rules' clock starts here, so start-up work
/// before it (index rebuild, spool drain) never counts against the limits.
async fn repeat_health(service: Service, every: Duration, limits: Limits, stop: CancellationToken) {
    let mut monitor = Monitor::new(limits, std::time::Instant::now());
    let mut sampler = ProcessSampler::default();
    loop {
        tokio::select! {
            () = stop.cancelled() => return,
            () = tokio::time::sleep(every) => service.sample_health(&mut monitor, &mut sampler),
        }
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
