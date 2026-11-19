//! The main window: the dashboard served by the daemon, created on demand and hidden (not closed)
//! when the user closes it, so capture keeps running in the tray.
//!
//! Every show and hide of a window goes through [`reveal`] and [`dismiss`], which tell its page
//! ([`VISIBLE_EVENT`]): a hidden page closes its live stream and timers, and refreshes once when
//! shown again (`platform.onWindowVisible`).

use tauri::{AppHandle, Emitter, Manager, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

use gitprompt_tauri::{DaemonStatus, GitPromptExt, Navigation, encode, navigation, open_in_browser};

use crate::{badge, perf};

pub(crate) const MAIN: &str = "main";

/// Sent to a window's page with `true` when the window is shown, `false` when it is hidden.
const VISIBLE_EVENT: &str = "gitprompt://visible";

/// Sent to the main window's page with a dashboard path to go to (`platform.onNavigate`).
const NAVIGATE_EVENT: &str = "gitprompt://navigate";

/// Show the main window, creating it once the daemon's start settled.
pub(crate) fn show<R: Runtime>(app: &AppHandle<R>) {
    show_at(app, None);
}

/// Show the main window on the dashboard path `route` (`/prompts?id=7`), or where it is when
/// `None`: a tray item or a notification that leads somewhere.
pub(crate) fn show_at<R: Runtime>(app: &AppHandle<R>, route: Option<&str>) {
    if let Some(window) = app.get_webview_window(MAIN) {
        go_to(&window, route);
        return;
    }
    let Some(gitprompt) = app.gitprompt() else { return };
    let app = app.clone();
    let route = route.map(str::to_owned);
    tauri::async_runtime::spawn(async move {
        let status = gitprompt.ready().await;
        perf::mark(&app, "daemonReady");
        if let Some(window) = app.get_webview_window(MAIN) {
            go_to(&window, route.as_deref()); // created meanwhile by another call
            return;
        }
        let page = |home: Url| match route.as_deref() {
            Some(route) => home.join(route).ok(),
            None => Some(home),
        };
        let url = if status.serving() { gitprompt.url().parse::<Url>().ok().and_then(page) } else { error_page(&status) };
        let Some(url) = url else {
            tracing::error!(status = ?status, "no page to show");
            return;
        };
        let home = gitprompt.url().parse::<Url>().ok();
        let builder = WebviewWindowBuilder::new(&app, MAIN, WebviewUrl::External(url))
            .on_navigation(move |target| home.as_ref().is_some_and(|home| on_navigation(home, target)))
            .title("GitPrompt")
            .inner_size(1280.0, 840.0)
            .min_inner_size(720.0, 480.0);
        let built = perf::configure(&app, builder).build();
        perf::mark(&app, "window");
        match built {
            Ok(window) => badge::apply(&window),
            Err(e) => tracing::error!(error = %e, "could not open the GitPrompt window"),
        }
    });
}

/// Reveal an existing main window and, with a route, take its page there.
fn go_to<R: Runtime>(window: &WebviewWindow<R>, route: Option<&str>) {
    reveal(window);
    if let Some(route) = route
        && let Err(e) = window.emit_to(MAIN, NAVIGATE_EVENT, route)
    {
        tracing::warn!(error = %e, route, "could not take the window to the page");
    }
}

/// Whether the window follows a link: the daemon's own pages stay in it, other web pages open in
/// the default browser instead, anything else goes nowhere (`gitprompt_tauri::navigation`).
fn on_navigation(home: &Url, target: &Url) -> bool {
    match navigation(home, target) {
        Navigation::Stay => true,
        Navigation::Browser => {
            if let Err(e) = open_in_browser(target) {
                tracing::warn!(error = %e, url = %target, "could not open the link in the browser");
            }
            false
        }
        Navigation::Refuse => false,
    }
}

/// Show a window, give it the keyboard and tell its page.
pub(crate) fn reveal<R: Runtime>(window: &WebviewWindow<R>) {
    // Each step is best effort: a window that can't unminimize can still be shown.
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    tell_page(window, true);
}

/// Tell a window's page whether it can be seen.
fn tell_page<R: Runtime>(window: &WebviewWindow<R>, visible: bool) {
    if let Err(e) = window.emit_to(window.label(), VISIBLE_EVENT, visible) {
        tracing::warn!(error = %e, window = window.label(), "could not tell the page it was shown or hidden");
    }
}

/// Hide a window and tell its page. False when it could not be hidden.
pub(crate) fn conceal<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    if let Err(e) = window.hide() {
        tracing::warn!(error = %e, window = window.label(), "could not hide the window");
        return false;
    }
    tell_page(window, false);
    true
}

/// [`conceal`] a window. When it was the app's last visible one, a Mac app hides too, so the
/// keyboard goes back to the app the user came from (the recall window opens over another app, and
/// the text it copied is pasted there).
pub(crate) fn dismiss<R: Runtime>(window: &WebviewWindow<R>) {
    if !conceal(window) {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let app = window.app_handle();
        let any_visible = app.webview_windows().values().any(|w| w.is_visible().unwrap_or(false));
        if !any_visible && let Err(e) = app.hide() {
            tracing::warn!(error = %e, "could not hand the keyboard back");
        }
    }
}

/// The dashboard's `platform.hideWindow()`: hide the window that asked.
#[tauri::command]
pub(crate) async fn hide_window(window: tauri::WebviewWindow) {
    dismiss(&window);
}

/// Closing the main window hides it; the app (and capture) keep running in the tray.
pub(crate) fn on_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == MAIN
    {
        api.prevent_close();
        if let Some(window) = window.get_webview_window(MAIN) {
            conceal(&window);
        }
    }
}

/// A one-line page saying why there is no dashboard (the daemon could not start).
fn error_page(status: &DaemonStatus) -> Option<Url> {
    let detail = match status {
        DaemonStatus::Failed { detail } => detail.as_str(),
        _ => "GitPrompt is not running.",
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>GitPrompt</title>\
         <body style=\"font:15px system-ui;padding:2rem\"><h1>GitPrompt could not start</h1><p>{}</p>\
         <p>Quit GitPrompt from the tray and start it again.</p>",
        escape(detail)
    );
    Url::parse(&format!("data:text/html;charset=utf-8,{}", encode(&html))).ok()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_page_shows_the_detail_escaped() {
        let url = error_page(&DaemonStatus::Failed { detail: "port <7878> in use".into() }).unwrap();
        assert_eq!(url.scheme(), "data");
        assert!(url.as_str().contains("%26lt%3B7878%26gt%3B"), "{url}");
    }
}
