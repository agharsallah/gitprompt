//! [`GitPrompt`]: the plugin's managed state — the settings, the daemon (while this app runs it)
//! and its [`DaemonStatus`].

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use gitprompt_core::Settings;
use gitprompt_server::{Daemon, Service};

use crate::error::CommandError;
use crate::native::Opener;

/// Who serves GitPrompt for this data directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum DaemonStatus {
    /// The in-process daemon is starting (lock, index, spool drain, first retention pass).
    Starting,
    /// This app runs the daemon.
    Running,
    /// Another process (a CLI `gitprompt serve`) holds the data directory's lock and serves it;
    /// the dashboard works against its URL, the native commands that need the store don't.
    External,
    /// The daemon could not start (port in use, unreadable store...).
    Failed { detail: String },
    /// Shut down (the app is exiting).
    Stopped,
}

impl DaemonStatus {
    /// Whether the start has settled (anything but [`DaemonStatus::Starting`]).
    pub fn settled(&self) -> bool {
        !matches!(self, DaemonStatus::Starting)
    }

    /// Whether a daemon (this app's or another process's) serves the dashboard URL.
    pub fn serving(&self) -> bool {
        matches!(self, DaemonStatus::Running | DaemonStatus::External)
    }
}

/// The plugin's state. Cheap to clone (shared).
#[derive(Clone)]
pub struct GitPrompt {
    inner: Arc<Inner>,
}

struct Inner {
    settings: Settings,
    opener: Arc<dyn Opener>,
    status: watch::Sender<DaemonStatus>,
    daemon: Mutex<Option<Daemon>>,
}

impl GitPrompt {
    pub(crate) fn new(settings: Settings, opener: Arc<dyn Opener>) -> Self {
        let (status, _) = watch::channel(DaemonStatus::Starting);
        GitPrompt { inner: Arc::new(Inner { settings, opener, status, daemon: Mutex::new(None) }) }
    }

    /// The settings the daemon was started with.
    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    pub(crate) fn opener(&self) -> &dyn Opener {
        self.inner.opener.as_ref()
    }

    pub fn status(&self) -> DaemonStatus {
        self.inner.status.borrow().clone()
    }

    /// Wait until the start settled; the status then.
    pub async fn ready(&self) -> DaemonStatus {
        let mut status = self.inner.status.subscribe();
        // `wait_for` only fails when the sender is gone, and `self` holds it.
        match status.wait_for(DaemonStatus::settled).await {
            Ok(settled) => settled.clone(),
            Err(_) => self.status(),
        }
    }

    /// Where the dashboard and the receiver are: the bound address while this app runs the daemon
    /// (the real port when configured with port 0), else `127.0.0.1:<settings.port>`.
    pub fn addr(&self) -> SocketAddr {
        self.daemon().as_ref().map_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, self.settings().port)), Daemon::local_addr)
    }

    /// `http://127.0.0.1:<port>/`: what the window loads.
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr())
    }

    /// The service facade of the in-process daemon. Fails with 409 when another process serves
    /// the data directory and 503 while starting, after a failed start, or after shutdown.
    pub fn service(&self) -> Result<Service, CommandError> {
        if let Some(daemon) = self.daemon().as_ref() {
            return Ok(daemon.service());
        }
        Err(match self.status() {
            DaemonStatus::External => CommandError::new(
                409,
                format!("GitPrompt is running in another process (gitprompt serve); use the dashboard at {}", self.url()),
            ),
            DaemonStatus::Starting => CommandError::new(503, "GitPrompt is starting"),
            DaemonStatus::Failed { detail } => CommandError::new(503, format!("GitPrompt could not start: {detail}")),
            DaemonStatus::Running | DaemonStatus::Stopped => CommandError::new(503, "GitPrompt is not running"),
        })
    }

    /// Record a started daemon. Hands it back when the app already began exiting, for the caller
    /// to shut it down.
    pub(crate) fn started(&self, daemon: Daemon) -> Result<(), Daemon> {
        let mut slot = self.daemon();
        if self.status() == DaemonStatus::Stopped {
            return Err(daemon);
        }
        *slot = Some(daemon);
        self.inner.status.send_replace(DaemonStatus::Running);
        Ok(())
    }

    /// Record a start that settled without a daemon of ours (external or failed).
    pub(crate) fn settle(&self, status: DaemonStatus) {
        self.inner.status.send_if_modified(|current| {
            let change = *current == DaemonStatus::Starting;
            if change {
                *current = status;
            }
            change
        });
    }

    /// Stop the in-process daemon (if any) and wait for it: in-flight requests finish, the store
    /// closes, the lock is released. The status is [`DaemonStatus::Stopped`] afterwards (an
    /// external daemon keeps running).
    pub async fn shutdown(&self) {
        let daemon = {
            let mut slot = self.daemon();
            self.inner.status.send_replace(DaemonStatus::Stopped);
            slot.take()
        };
        if let Some(daemon) = daemon {
            match daemon.shutdown().await {
                Ok(()) => tracing::info!("GitPrompt stopped"),
                Err(e) => tracing::warn!(error = %e, "GitPrompt did not stop cleanly"),
            }
        }
    }

    fn daemon(&self) -> MutexGuard<'_, Option<Daemon>> {
        // Nothing panics while holding the lock; a poisoned slot is still consistent.
        self.inner.daemon.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl std::fmt::Debug for GitPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitPrompt").field("home", &self.settings().home).field("status", &self.status()).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::SystemOpener;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn state() -> GitPrompt {
        let mut settings = Settings::new("/nowhere/.gitprompt", "/nowhere");
        settings.port = 9123;
        GitPrompt::new(settings, Arc::new(SystemOpener))
    }

    #[test]
    fn status_serializes_with_a_state_tag() {
        assert_eq!(serde_json::to_value(DaemonStatus::Running).unwrap(), json!({"state": "running"}));
        assert_eq!(serde_json::to_value(DaemonStatus::External).unwrap(), json!({"state": "external"}));
        assert_eq!(
            serde_json::to_value(DaemonStatus::Failed { detail: "port in use".into() }).unwrap(),
            json!({"state": "failed", "detail": "port in use"})
        );
    }

    #[test]
    fn starts_as_starting_on_the_configured_port() {
        let state = state();
        assert_eq!(state.status(), DaemonStatus::Starting);
        assert_eq!(state.url(), "http://127.0.0.1:9123/");
        assert_eq!(state.service().err().unwrap().status, 503);
    }

    #[test]
    fn an_external_daemon_refuses_store_commands_with_409() {
        let state = state();
        state.settle(DaemonStatus::External);
        let error = state.service().err().unwrap();
        assert_eq!(error.status, 409);
        assert!(error.detail.contains("http://127.0.0.1:9123/"), "{}", error.detail);
        assert!(state.status().serving());
    }

    #[test]
    fn a_failed_start_reports_why() {
        let state = state();
        state.settle(DaemonStatus::Failed { detail: "port in use".into() });
        assert_eq!(state.service().err().unwrap(), CommandError::new(503, "GitPrompt could not start: port in use"));
        assert!(!state.status().serving());
    }

    #[test]
    fn settle_only_changes_a_starting_status() {
        let state = state();
        state.settle(DaemonStatus::External);
        state.settle(DaemonStatus::Failed { detail: "late".into() });
        assert_eq!(state.status(), DaemonStatus::External);
    }

    #[test]
    fn ready_returns_the_settled_status() {
        let state = state();
        let waiter = state.clone();
        let handle = std::thread::spawn(move || tauri::async_runtime::block_on(waiter.ready()));
        state.settle(DaemonStatus::External);
        assert_eq!(handle.join().unwrap(), DaemonStatus::External);
    }

    #[test]
    fn shutdown_without_a_daemon_marks_it_stopped() {
        let state = state();
        tauri::async_runtime::block_on(state.shutdown());
        assert_eq!(state.status(), DaemonStatus::Stopped);
        assert!(state.status().settled());
    }
}
