//! The Dock (taskbar) badge: the possible leaks not handled yet, set by the tray with each redraw.
//! It belongs to the app but is set through a window, so the count is kept and given to a window
//! created later. Windows has no badge.

use std::sync::{Mutex, PoisonError};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// The count shown now.
#[derive(Default)]
struct Badge(Mutex<Option<i64>>);

pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.manage(Badge::default());
}

/// Show `count` on the app's icon (`None`: no badge).
pub(crate) fn set<R: Runtime>(app: &AppHandle<R>, count: Option<i64>) {
    let Some(badge) = app.try_state::<Badge>() else { return };
    let mut shown = badge.0.lock().unwrap_or_else(PoisonError::into_inner);
    if *shown == count {
        return;
    }
    *shown = count;
    if let Some(window) = app.webview_windows().into_values().next() {
        put(&window, count);
    }
}

/// A new window: give it the count shown now (the app may have had no window to set it on).
pub(crate) fn apply<R: Runtime>(window: &WebviewWindow<R>) {
    let count = window.try_state::<Badge>().and_then(|badge| *badge.0.lock().unwrap_or_else(PoisonError::into_inner));
    put(window, count);
}

fn put<R: Runtime>(window: &WebviewWindow<R>, count: Option<i64>) {
    if let Err(e) = window.set_badge_count(count) {
        tracing::debug!(error = %e, "no badge on this system");
    }
}
