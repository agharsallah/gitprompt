//! The main window: the dashboard served by the daemon, created on demand and hidden (not closed)
//! when the user closes it, so capture keeps running in the tray.

use tauri::{AppHandle, Manager, Runtime, Url, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};

use gitprompt_tauri::{DaemonStatus, GitPromptExt};

pub(crate) const MAIN: &str = "main";

/// Show the main window, creating it once the daemon's start settled.
pub(crate) fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN) {
        reveal(&window);
        return;
    }
    let Some(gitprompt) = app.gitprompt() else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = gitprompt.ready().await;
        if let Some(window) = app.get_webview_window(MAIN) {
            reveal(&window); // created meanwhile by another call
            return;
        }
        let url = if status.serving() { gitprompt.url().parse::<Url>().ok() } else { error_page(&status) };
        let Some(url) = url else {
            tracing::error!(status = ?status, "no page to show");
            return;
        };
        let built = WebviewWindowBuilder::new(&app, MAIN, WebviewUrl::External(url))
            .title("GitPrompt")
            .inner_size(1280.0, 840.0)
            .min_inner_size(720.0, 480.0)
            .build();
        if let Err(e) = built {
            tracing::error!(error = %e, "could not open the GitPrompt window");
        }
    });
}

fn reveal<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    // Each step is best effort: a window that can't unminimize can still be shown.
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Closing the main window hides it; the app (and capture) keep running in the tray.
pub(crate) fn on_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == MAIN
    {
        api.prevent_close();
        if let Err(e) = window.hide() {
            tracing::warn!(error = %e, "could not hide the GitPrompt window");
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
    Url::parse(&format!("data:text/html;charset=utf-8,{}", percent_encode(&html))).ok()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
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
