//! Where a link clicked in the app window goes. The window shows the dashboard served by the
//! daemon, so a link to GitHub would replace the dashboard inside the window with no way back: only
//! the daemon's own pages stay in it, web pages open in the default browser, anything else is
//! refused.

use std::io;

use tauri::Url;

/// What to do with a navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    /// The daemon's own page (same scheme, host and port), or the app's own error page (`data:`,
    /// `about:blank`): load it in the window.
    Stay,
    /// Another web page (`http`, `https`): open it in the default browser.
    Browser,
    /// Anything else (`javascript:`, `file:`, other schemes): do nothing.
    Refuse,
}

/// The decision for a navigation to `target` in a window showing the daemon at `daemon`.
pub fn navigation(daemon: &Url, target: &Url) -> Navigation {
    if target.origin() == daemon.origin() && target.origin().is_tuple() {
        return Navigation::Stay;
    }
    match target.scheme() {
        "http" | "https" => Navigation::Browser,
        "data" => Navigation::Stay,
        "about" if target.path() == "blank" => Navigation::Stay,
        _ => Navigation::Refuse,
    }
}

/// Open a web page in the default browser without waiting for it, with the platform's own opener
/// (`open`, `explorer`, `xdg-open`: the commands that open folders, `crate::native`). Only `http`
/// and `https` URLs are opened.
pub fn open_in_browser(url: &Url) -> io::Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("not a web page: {url}")));
    }
    crate::native::open_url(url.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn only_the_daemons_origin_stays_in_the_window() {
        let daemon = Url::parse("http://127.0.0.1:7878/").unwrap();
        let cases = [
            ("http://127.0.0.1:7878/projects/abc?tab=commits#commit-a1b2c3d", Navigation::Stay),
            ("http://127.0.0.1:7878/", Navigation::Stay),
            ("data:text/html,<h1>GitPrompt could not start</h1>", Navigation::Stay),
            ("about:blank", Navigation::Stay),
            ("http://127.0.0.1:9999/", Navigation::Browser),
            ("http://localhost:7878/", Navigation::Browser),
            ("https://github.com/acme/app/commit/a1b2c3d", Navigation::Browser),
            ("javascript:alert(1)", Navigation::Refuse),
            ("file:///etc/passwd", Navigation::Refuse),
            ("vscode://file/x", Navigation::Refuse),
        ];
        for (target, expected) in cases {
            assert_eq!(navigation(&daemon, &Url::parse(target).unwrap()), expected, "{target}");
        }
    }

    #[test]
    fn only_web_pages_open_in_the_browser() {
        let refused = open_in_browser(&Url::parse("file:///etc/passwd").unwrap()).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
    }
}
