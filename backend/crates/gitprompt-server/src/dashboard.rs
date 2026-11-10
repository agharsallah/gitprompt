//! The built dashboard (SvelteKit static build), served at `/` from a [`Dashboard`]: a folder on
//! disk or the copy compiled into the binary ([`gitprompt_dashboard`]).
//!
//! * A page path with no file of its own (a story: `/sessions/<id>/`) gets the build's fallback page
//!   [`FALLBACK`], which routes in the browser. Never for `/api/**` and `/ingest/**` (they keep their
//!   404), nor for a path whose last segment has a file extension (a missing asset is a 404, not an
//!   HTML page). A folder path serves its `index.html`.
//! * When the dashboard was not built (no `index.html`), `/` and every fallback path get
//!   [`MISSING_PAGE`] instead, which says how to build it; the API keeps working.
//! * Caching: hashed assets under [`IMMUTABLE`] never change (`public, max-age=31536000,
//!   immutable`); HTML pages are revalidated on every load (`no-cache`), so a new build shows at once.

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use gitprompt_core::Env;

use crate::error::ApiError;

/// The build's client-side routing page.
pub const FALLBACK: &str = "200.html";

/// The page every build has; without it the dashboard counts as not built.
pub const INDEX: &str = "index.html";

/// Where SvelteKit puts content-hashed assets.
pub const IMMUTABLE: &str = "_app/immutable/";

/// The variable naming a dashboard folder to serve instead of the embedded one.
pub const DASHBOARD_ENV: &str = "GITPROMPT_DASHBOARD";

const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const CACHE_HTML: &str = "no-cache";

/// Served at `/` (and for fallback paths) when the dashboard was not built.
pub const MISSING_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>GitPrompt</title>
<style>
  body { font: 16px/1.5 system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; padding: 0 1rem; color: #222; background: #fff; }
  code { background: #f2f2f2; padding: 0.1rem 0.3rem; border-radius: 3px; }
  @media (prefers-color-scheme: dark) { body { color: #ddd; background: #161616; } code { background: #2a2a2a; } }
</style>
</head>
<body>
<h1>The dashboard isn't built</h1>
<p>GitPrompt is running and capturing prompts, but this binary has no dashboard to show.</p>
<p>In the repository, run <code>just build</code> (or <code>pnpm --dir web build</code>), then reload this page.
A release build needs a rebuild of the binary afterwards; a debug build picks the files up at once.
<code>GITPROMPT_DASHBOARD=&lt;folder&gt;</code> serves a built dashboard from another folder.</p>
<p>The API answers as usual: <a href="/api/status">/api/status</a>, <a href="/healthz">/healthz</a>.</p>
</body>
</html>
"#;

/// Where the dashboard's files come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dashboard {
    /// A built dashboard folder (`web/build`), read on every request.
    Dir(PathBuf),
    /// The copy compiled into the binary ([`gitprompt_dashboard`]; read from `web/build` at run
    /// time in debug builds).
    Embedded,
}

impl Dashboard {
    /// [`DASHBOARD_ENV`] when it is set (`~` expanded against `HOME`), else [`Dashboard::Embedded`].
    pub fn from_env(env: &Env) -> Self {
        match env.get(DASHBOARD_ENV).map(str::trim).filter(|dir| !dir.is_empty()) {
            Some(dir) => Dashboard::Dir(expand_home(dir, env)),
            None => Dashboard::Embedded,
        }
    }

    /// What it serves, for `/api/status` and `gitprompt doctor`.
    pub fn info(&self) -> DashboardInfo {
        match self {
            Dashboard::Dir(dir) => {
                DashboardInfo { source: DashboardSource::Dir, dir: Some(dir.display().to_string()), built: dir.join(INDEX).is_file() }
            }
            Dashboard::Embedded => DashboardInfo {
                source: DashboardSource::Embedded,
                dir: gitprompt_dashboard::READS_FROM_DISK.then(|| gitprompt_dashboard::build_dir().display().to_string()),
                built: gitprompt_dashboard::is_built(),
            },
        }
    }

    /// The bytes of `path` (relative, `/`-separated); `None` for a missing file or a folder.
    /// Blocking: reads the disk for a folder, and for the embedded copy in debug builds.
    fn read(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        match self {
            Dashboard::Dir(dir) => std::fs::read(dir.join(path)).ok().map(Cow::Owned),
            Dashboard::Embedded => gitprompt_dashboard::get(path),
        }
    }
}

/// `~` or `~/rest` against the environment's `HOME`; anything else as given.
fn expand_home(dir: &str, env: &Env) -> PathBuf {
    let home = env.get("HOME").or_else(|| env.get("USERPROFILE"));
    match (dir.strip_prefix('~'), home) {
        (Some(""), Some(home)) => PathBuf::from(home),
        (Some(rest), Some(home)) if rest.starts_with('/') => Path::new(home).join(rest.trim_start_matches('/')),
        _ => PathBuf::from(dir),
    }
}

/// Which kind of [`Dashboard`] a server uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DashboardSource {
    Embedded,
    Dir,
    /// No dashboard: API only.
    None,
}

/// The dashboard a server serves, as `/api/status` reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardInfo {
    pub source: DashboardSource,
    /// The folder read: the [`Dashboard::Dir`] folder, or `web/build` for the embedded copy of a
    /// debug build (which reads it at run time). `None` for a release build's embedded copy.
    pub dir: Option<String>,
    /// Whether there is a built dashboard (an `index.html`); `false` serves [`MISSING_PAGE`].
    pub built: bool,
}

impl DashboardInfo {
    /// No dashboard (`AppOptions::dashboard` is `None`).
    pub fn none() -> Self {
        DashboardInfo { source: DashboardSource::None, dir: None, built: false }
    }

    /// One line for logs and `gitprompt doctor`: `embedded`, `embedded (not built)`,
    /// `dir /path`, `dir /path (not built)`, `none (API only)`.
    pub fn describe(&self) -> String {
        let base = match (self.source, &self.dir) {
            (DashboardSource::None, _) => return "none (API only)".to_owned(),
            (DashboardSource::Embedded, None) => "embedded".to_owned(),
            (DashboardSource::Embedded, Some(dir)) => format!("embedded (debug build, read from {dir})"),
            (DashboardSource::Dir, dir) => format!("dir {}", dir.as_deref().unwrap_or_default()),
        };
        if self.built { base } else { format!("{base} (not built)") }
    }
}

/// Whether a request path that matched no file gets [`FALLBACK`].
pub fn serves_fallback(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    let first = path.split('/').next().unwrap_or_default();
    let last = path.rsplit('/').next().unwrap_or_default();
    !matches!(first, "api" | "ingest") && !last.contains('.')
}

/// The files of `dashboard` with the fallback rule, as the router's fallback service.
pub fn router(dashboard: Dashboard) -> Router {
    let dashboard = Arc::new(dashboard);
    Router::new().fallback(move |request: Request| {
        let dashboard = Arc::clone(&dashboard);
        async move { serve(dashboard, request.uri().path().to_owned()).await }
    })
}

/// What a request path resolves to.
enum Lookup {
    /// A file of the build (`name` relative to it).
    File { name: String, bytes: Cow<'static, [u8]> },
    /// No build at all: [`MISSING_PAGE`].
    NotBuilt,
    /// 404.
    Absent,
}

async fn serve(dashboard: Arc<Dashboard>, path: String) -> Response {
    let found = tokio::task::spawn_blocking(move || resolve(&dashboard, &path)).await.unwrap_or(Lookup::Absent);
    match found {
        Lookup::File { name, bytes } => file_response(&name, bytes),
        Lookup::NotBuilt => file_response(INDEX, Cow::Borrowed(MISSING_PAGE.as_bytes())),
        Lookup::Absent => ApiError::NotFound("Not Found".into()).into_response(),
    }
}

/// The file for `path`: itself, its folder's `index.html`, else (when the rule allows) the fallback
/// page, or the "not built" page when there is no build at all.
fn resolve(dashboard: &Dashboard, path: &str) -> Lookup {
    let Some(relative) = relative(path) else { return Lookup::Absent };
    let candidates = if relative.is_empty() || path.ends_with('/') {
        vec![join(&relative, INDEX)]
    } else {
        vec![relative.clone(), join(&relative, INDEX)]
    };
    for name in candidates {
        if let Some(bytes) = dashboard.read(&name) {
            return Lookup::File { name, bytes };
        }
    }
    if !serves_fallback(path) {
        return Lookup::Absent;
    }
    match dashboard.read(FALLBACK) {
        Some(bytes) => Lookup::File { name: FALLBACK.to_owned(), bytes },
        None if dashboard.read(INDEX).is_none() => Lookup::NotBuilt,
        None => Lookup::Absent,
    }
}

/// `dir/name`, or `name` for the root.
fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}

fn file_response(name: &str, bytes: Cow<'static, [u8]>) -> Response {
    let mut response = Body::from(bytes).into_response();
    let headers = response.headers_mut();
    let media_type = media_type(name);
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    if let Some(cache) = cache_control(name, media_type) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    }
    response
}

/// Immutable for hashed assets, `no-cache` for HTML, nothing (the browser's heuristics) otherwise.
fn cache_control(name: &str, media_type: &str) -> Option<&'static str> {
    if name.starts_with(IMMUTABLE) {
        Some(CACHE_IMMUTABLE)
    } else if media_type.starts_with("text/html") {
        Some(CACHE_HTML)
    } else {
        None
    }
}

/// The request path as a relative, `/`-separated file path: percent-decoded, `..` refused (`None`),
/// empty segments dropped. `""` for the root.
fn relative(path: &str) -> Option<String> {
    let decoded = crate::query::pairs(Some(&format!("p={}", path.replace('+', "%2B")))).pop()?.1;
    let relative = PathBuf::from(decoded.trim_start_matches('/'));
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
            _ => return None,
        }
    }
    // A backslash would be a separator on Windows: refuse it rather than reinterpret it.
    if parts.iter().any(|part| part.contains('\\')) {
        return None;
    }
    Some(parts.join("/"))
}

fn media_type(file: &str) -> &'static str {
    match Path::new(file).extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn relative_paths_are_slash_joined_and_never_leave_the_folder() {
        assert_eq!(relative("/"), Some(String::new()));
        assert_eq!(relative("/sessions/"), Some("sessions".into()));
        assert_eq!(relative("/_app//immutable/a.js"), Some("_app/immutable/a.js".into()));
        assert_eq!(relative("/a%20b.txt"), Some("a b.txt".into()));
        assert_eq!(relative("/../secret"), None);
        assert_eq!(relative("/a/%2e%2e/%2e%2e/secret"), None);
        assert_eq!(relative("/a%5c..%5csecret"), None);
    }

    #[test]
    fn hashed_assets_are_immutable_and_html_is_revalidated() {
        assert_eq!(cache_control("_app/immutable/chunks/a.js", "text/javascript"), Some(CACHE_IMMUTABLE));
        assert_eq!(cache_control("index.html", media_type("index.html")), Some(CACHE_HTML));
        assert_eq!(cache_control("favicon.png", media_type("favicon.png")), None);
        assert_eq!(cache_control("_app/version.json", media_type("_app/version.json")), None);
    }

    #[test]
    fn the_dashboard_variable_overrides_the_embedded_copy() {
        let env = |pairs: &[(&str, &str)]| Env::from_pairs(pairs.iter().copied());
        assert_eq!(Dashboard::from_env(&env(&[])), Dashboard::Embedded);
        assert_eq!(Dashboard::from_env(&env(&[(DASHBOARD_ENV, " ")])), Dashboard::Embedded);
        assert_eq!(Dashboard::from_env(&env(&[(DASHBOARD_ENV, "/srv/web")])), Dashboard::Dir("/srv/web".into()));
        assert_eq!(Dashboard::from_env(&env(&[("HOME", "/u"), (DASHBOARD_ENV, "~/web")])), Dashboard::Dir("/u/web".into()));
    }

    #[test]
    fn describe_names_the_source_and_whether_it_is_built() {
        let dir = DashboardInfo { source: DashboardSource::Dir, dir: Some("/w".into()), built: true };
        assert_eq!(dir.describe(), "dir /w");
        assert_eq!(DashboardInfo { built: false, ..dir }.describe(), "dir /w (not built)");
        assert_eq!(DashboardInfo { source: DashboardSource::Embedded, dir: None, built: true }.describe(), "embedded");
        assert_eq!(DashboardInfo::none().describe(), "none (API only)");
    }
}
