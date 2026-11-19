//! The recall window: a small frameless window on top of everything, built the first time it is
//! wanted and then kept hidden, so showing it again is a move and a show. It opens on the screen
//! with the pointer and goes away on blur (see [`crate::recall::on_event`]).

use tauri::{AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use gitprompt_tauri::GitPromptExt;

use super::Recall;
use super::place::{Area, top_center};
use crate::window;

pub(crate) const LABEL: &str = "recall";

/// The window's inner size in logical pixels.
const SIZE: (f64, f64) = (640.0, 420.0);

/// The dashboard in recall mode: the palette alone, finding prompts only.
fn recall_url(dashboard: &str) -> Option<Url> {
    let mut url = Url::parse(dashboard).ok()?;
    url.set_query(Some("recall=1"));
    Some(url)
}

/// The shortcut was pressed: show the window, or hide it when it is showing.
pub(crate) fn toggle<R: Runtime>(app: &AppHandle<R>) {
    match app.get_webview_window(LABEL) {
        Some(window) if window.is_visible().unwrap_or(false) => window::dismiss(&window),
        Some(window) => reveal(app, &window),
        None => build_and_reveal(app),
    }
}

/// First use: wait for the daemon (the page it serves is the window), build the window hidden,
/// then show it like every later time.
fn build_and_reveal<R: Runtime>(app: &AppHandle<R>) {
    let Some(gitprompt) = app.gitprompt() else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = gitprompt.ready().await;
        if let Some(window) = app.get_webview_window(LABEL) {
            return reveal(&app, &window); // built meanwhile by another press
        }
        let Some(url) = status.serving().then(|| recall_url(&gitprompt.url())).flatten() else {
            tracing::warn!(status = ?status, "the recall window needs the dashboard, which is not running");
            return;
        };
        match build(&app, url) {
            Ok(window) => reveal(&app, &window),
            Err(e) => tracing::error!(error = %e, "could not build the recall window"),
        }
    });
}

fn build<R: Runtime>(app: &AppHandle<R>, url: Url) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("GitPrompt recall")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        // Off the taskbar (Windows, Linux). macOS keeps one Dock icon per app, not per window.
        .skip_taskbar(true)
        // Opens on the desktop you are looking at, not the one the main window was left on.
        .visible_on_all_workspaces(true)
        .visible(false)
        .build()
}

/// Move the window under the pointer, show it and give it the keyboard.
fn reveal<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    if let Some(recall) = app.try_state::<Recall>() {
        recall.reset();
    }
    place(app, window);
    // A window that can't be moved can still be shown.
    window::reveal(window);
}

fn place<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Some(monitor) = pointer_monitor(app) else {
        let _ = window.center();
        return;
    };
    let work = monitor.work_area();
    let area = Area { x: work.position.x, y: work.position.y, width: work.size.width, height: work.size.height };
    let size = LogicalSize::new(SIZE.0, SIZE.1).to_physical::<u32>(monitor.scale_factor());
    let (x, y) = top_center(area, (size.width, size.height));
    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        tracing::warn!(error = %e, "could not move the recall window");
    }
}

/// The screen with the pointer, else the main one.
fn pointer_monitor<R: Runtime>(app: &AppHandle<R>) -> Option<Monitor> {
    let at_pointer = app.cursor_position().ok().and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    at_pointer.or_else(|| app.primary_monitor().ok().flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recall_page_is_the_dashboard_root_in_recall_mode() {
        let url = recall_url("http://127.0.0.1:7878/").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:7878/?recall=1");
    }

    #[test]
    fn a_dashboard_address_that_is_not_a_url_gives_no_page() {
        assert!(recall_url("not a url").is_none());
    }
}
