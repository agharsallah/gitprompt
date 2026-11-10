//! Starting the daemon at setup, forwarding its bus to Tauri events, stopping it at exit.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Runtime};

use gitprompt_server::{AppOptions, Daemon, DaemonError, DaemonOptions, Service};

use crate::GitPromptExt;
use crate::state::{DaemonStatus, GitPrompt};

/// Emitted with `{prompts, events}` each time something is captured (the SSE notice).
pub const CAPTURED_EVENT: &str = "gitprompt://captured";

/// Emitted with the [`DaemonStatus`] when the start settles.
pub const STATUS_EVENT: &str = "gitprompt://status";

/// How long the exit waits for the daemon to stop (in-flight requests, store close).
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Start the daemon, record the outcome in `state`, emit [`STATUS_EVENT`] and, when this app
/// runs the daemon, forward its notices as [`CAPTURED_EVENT`].
pub(crate) async fn start<R: Runtime>(app: AppHandle<R>, state: GitPrompt, options: AppOptions, daemon: DaemonOptions) {
    match Daemon::start(state.settings().clone(), options, daemon).await {
        Ok(daemon) => {
            let service = daemon.service();
            match state.started(daemon) {
                Ok(()) => {
                    tracing::info!(url = %state.url(), "GitPrompt daemon running in the desktop app");
                    tauri::async_runtime::spawn(forward(app.clone(), service));
                }
                Err(daemon) => {
                    // The app began exiting while the daemon started.
                    if let Err(e) = daemon.shutdown().await {
                        tracing::warn!(error = %e, "GitPrompt did not stop cleanly");
                    }
                }
            }
        }
        Err(DaemonError::AlreadyRunning { lock_file }) => {
            tracing::info!(lock = %lock_file.display(), url = %state.url(), "GitPrompt already runs in another process; using it");
            state.settle(DaemonStatus::External);
        }
        Err(e) => {
            tracing::error!(error = %e, "GitPrompt daemon could not start");
            state.settle(DaemonStatus::Failed { detail: e.to_string() });
        }
    }
    if let Err(e) = app.emit(STATUS_EVENT, state.status()) {
        tracing::warn!(error = %e, "could not emit the GitPrompt status");
    }
}

/// Every bus notice as a [`CAPTURED_EVENT`], until the daemon shuts down.
async fn forward<R: Runtime>(app: AppHandle<R>, service: Service) {
    let mut notices = service.bus().subscribe();
    let closing = service.closing();
    drop(service); // the store closes with the daemon, not with this task
    loop {
        tokio::select! {
            () = closing.cancelled() => return,
            notice = notices.next() => match notice {
                Some(notice) => {
                    if let Err(e) = app.emit(CAPTURED_EVENT, notice) {
                        tracing::warn!(error = %e, "could not emit a capture notice");
                    }
                }
                None => return,
            },
        }
    }
}

/// `RunEvent::Exit`: stop the daemon, waiting at most [`SHUTDOWN_GRACE`].
pub(crate) fn exit<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.gitprompt() else { return };
    tauri::async_runtime::block_on(async move {
        if tokio::time::timeout(SHUTDOWN_GRACE, state.shutdown()).await.is_err() {
            tracing::warn!("GitPrompt did not stop within {SHUTDOWN_GRACE:?}; exiting anyway");
        }
    });
}
