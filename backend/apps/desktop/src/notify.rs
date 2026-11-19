//! Native notifications: what the daemon decided deserves one (a long turn, a possible leak, a
//! connector gone quiet, the weekly recap), shown by the system. A click shows the window on the
//! notification's target (`gitprompt_tauri::route`; deep links replace this later).
//!
//! While the window is in front the page shows the same notification as a toast, so the system
//! one is skipped then.

use std::sync::atomic::{AtomicI32, Ordering};

use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use gitprompt_server::model::Notification;
use gitprompt_tauri::{NOTIFICATION_EVENT, route};

use crate::window;

/// The notification's extra field holding the dashboard path a click opens.
const ROUTE: &str = "route";

/// The action a click on the notification itself reports.
const TAP: &str = "tap";

/// The id of the next notification shown (each its own, so one never replaces another).
#[derive(Default)]
struct Shown(AtomicI32);

pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(Shown::default());
    let handle = app.clone();
    // Registered before any notification is shown: only those shown after it report clicks.
    let clicks = app.notification().on_action(move |performed| {
        if performed.action_id() != TAP {
            return;
        }
        let route = performed.notification().and_then(|n| n.extra().get(ROUTE)).and_then(serde_json::Value::as_str);
        window::show_at(&handle, route);
    });
    if let Err(e) = clicks {
        tracing::warn!(error = %e, "notification clicks will not open GitPrompt");
    }
    let handle = app.clone();
    app.listen_any(NOTIFICATION_EVENT, move |event| match serde_json::from_str::<Notification>(event.payload()) {
        Ok(notification) => show(&handle, &notification),
        Err(e) => tracing::warn!(error = %e, "an unreadable notification"),
    });
}

fn show<R: Runtime>(app: &AppHandle<R>, notification: &Notification) {
    if in_front(app) {
        return;
    }
    let id = app.state::<Shown>().0.fetch_add(1, Ordering::Relaxed);
    let shown = app
        .notification()
        .builder()
        .id(id)
        .title(&notification.title)
        .body(&notification.body)
        .extra(ROUTE, route(&notification.target))
        .show();
    if let Err(e) = shown {
        tracing::warn!(error = %e, kind = ?notification.kind, "could not show a notification");
    }
}

/// The main window is visible and has the keyboard: its page shows the toast.
fn in_front<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window(window::MAIN).is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
}
