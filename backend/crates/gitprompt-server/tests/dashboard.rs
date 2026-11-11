//! The static dashboard with its SPA fallback, and /healthz. Port of
//! `test_dashboard_serves_the_fallback_page_for_story_paths` (`test_api.py`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use std::path::Path;

use common::{Api, api, harness};
use gitprompt_server::Dashboard;
use gitprompt_server::dashboard::{MISSING_PAGE, serves_fallback};
use pretty_assertions::assert_eq;
use serde_json::json;

/// A built dashboard: home page, fallback page, a sessions index and an asset.
fn with_dashboard() -> Api {
    let h = harness();
    let dir = h.root().join("dashboard");
    std::fs::create_dir_all(dir.join("sessions")).expect("mkdir");
    std::fs::create_dir_all(dir.join("_app")).expect("mkdir");
    write(&dir, "index.html", "home");
    write(&dir, "200.html", "fallback");
    write(&dir, "sessions/index.html", "sessions");
    write(&dir, "_app/start.js", "console.log(1)");
    std::fs::create_dir_all(dir.join("_app/immutable/chunks")).expect("mkdir");
    write(&dir, "_app/immutable/chunks/a1b2.js", "export {}");
    h.dashboard(dir).start()
}

/// A dashboard folder with nothing built in it.
fn not_built() -> Api {
    let h = harness();
    let dir = h.root().join("empty-build");
    std::fs::create_dir_all(&dir).expect("mkdir");
    h.dashboard(dir).start()
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).expect("write");
}

async fn page(api: &Api, path: &str) -> (u16, String) {
    let response = api.get(path).send().await;
    (response.status, response.text())
}

#[tokio::test]
async fn the_home_page_and_folder_index_pages_are_served() {
    let api = with_dashboard();
    assert_eq!(page(&api, "/").await, (200, "home".to_string()));
    assert_eq!(page(&api, "/sessions/").await, (200, "sessions".to_string()));
}

#[tokio::test]
async fn an_existing_asset_is_served_as_is() {
    let api = with_dashboard();
    assert_eq!(page(&api, "/_app/start.js").await, (200, "console.log(1)".to_string()));
}

#[tokio::test]
async fn a_story_path_with_no_file_of_its_own_gets_the_fallback_page() {
    let api = with_dashboard();
    assert_eq!(page(&api, "/sessions/abc-123/").await, (200, "fallback".to_string()));
    assert_eq!(page(&api, "/projects/L3IvYXBw").await, (200, "fallback".to_string()));
}

#[tokio::test]
async fn api_and_ingest_paths_never_get_the_fallback_page() {
    let api = with_dashboard();
    let response = api.get("/api/nothing").send().await;
    assert_eq!((response.status, response.json()), (404, json!({"detail": "Not Found"})));
    let ingest = page(&api, "/ingest/claude-code/whatever").await;
    assert_eq!(ingest.0, 404);
    assert_ne!(ingest.1, "fallback");
}

#[tokio::test]
async fn a_missing_asset_is_404_not_the_fallback_page() {
    let api = with_dashboard();
    let (status, body) = page(&api, "/missing.js").await;
    assert_eq!(status, 404);
    assert_ne!(body, "fallback");
}

#[tokio::test]
async fn api_routes_still_answer_with_a_dashboard_mounted() {
    let api = with_dashboard();
    assert_eq!(api.json("/api/prompts").await, json!([]));
}

#[tokio::test]
async fn without_a_dashboard_unknown_paths_are_404() {
    let api = api();
    assert_eq!(api.get("/sessions/abc/").send().await.status, 404);
}

#[tokio::test]
async fn hashed_assets_are_cached_forever_and_pages_are_revalidated() {
    let api = with_dashboard();
    let asset = api.get("/_app/immutable/chunks/a1b2.js").send().await;
    assert_eq!(asset.status, 200);
    assert_eq!(asset.header("cache-control").as_deref(), Some("public, max-age=31536000, immutable"));
    assert_eq!(asset.header("content-type").as_deref(), Some("text/javascript; charset=utf-8"));
    for path in ["/", "/sessions/", "/sessions/abc-123/"] {
        let page = api.get(path).send().await;
        assert_eq!(page.header("cache-control").as_deref(), Some("no-cache"), "{path}");
        assert_eq!(page.header("content-type").as_deref(), Some("text/html; charset=utf-8"), "{path}");
    }
    assert_eq!(api.get("/_app/start.js").send().await.header("cache-control"), None);
}

#[tokio::test]
async fn a_dashboard_that_is_not_built_says_how_to_build_it() {
    let api = not_built();
    for path in ["/", "/sessions/abc-123/"] {
        let page = api.get(path).send().await;
        assert_eq!((page.status, page.text()), (200, MISSING_PAGE.to_string()), "{path}");
        assert_eq!(page.header("content-type").as_deref(), Some("text/html; charset=utf-8"));
        assert_eq!(page.header("cache-control").as_deref(), Some("no-cache"));
    }
    assert!(MISSING_PAGE.contains("just build"));
}

#[tokio::test]
async fn without_a_build_assets_and_the_api_keep_their_answers() {
    let api = not_built();
    assert_eq!(api.get("/missing.js").send().await.status, 404);
    assert_eq!(api.get("/api/nothing").send().await.status, 404);
    assert_eq!(api.json("/api/prompts").await, json!([]));
}

#[tokio::test]
async fn a_dashboard_folder_that_does_not_exist_is_not_built_either() {
    let h = harness();
    let dir = h.root().join("nowhere");
    let api = h.dashboard(dir).start();
    assert_eq!(page(&api, "/").await, (200, MISSING_PAGE.to_string()));
}

#[tokio::test]
async fn the_embedded_dashboard_serves_its_home_page_or_the_build_hint() {
    let mut h = harness();
    h.options.dashboard = Some(Dashboard::Embedded);
    let api = h.start();
    let home = api.get("/").send().await;
    assert_eq!(home.status, 200);
    assert_eq!(home.header("content-type").as_deref(), Some("text/html; charset=utf-8"));
    let built = gitprompt_dashboard::is_built();
    assert_eq!(home.text() == MISSING_PAGE, !built, "built: {built}");
    assert_eq!(api.get("/no-such-asset.js").send().await.status, 404);
}

#[tokio::test]
async fn paths_leaving_the_dashboard_folder_are_refused() {
    let api = with_dashboard();
    std::fs::write(api.root().join("secret.txt"), "secret").expect("write");
    for path in ["/../secret.txt", "/%2e%2e/secret.txt", "/sessions/%2e%2e/%2e%2e/secret.txt"] {
        let response = api.get(path).send().await;
        assert_ne!(response.text(), "secret", "{path}");
    }
}

#[tokio::test]
async fn healthz_answers_ok() {
    let api = api();
    let response = api.get("/healthz").send().await;
    assert_eq!((response.status, response.json()), (200, json!({"ok": true})));
}

#[test]
fn the_fallback_rule_spares_api_ingest_and_file_paths() {
    let cases = [
        ("/sessions/abc-123/", true),
        ("/sessions/abc-123", true),
        ("/projects/L3IvYXBw/", true),
        ("/", true),
        ("/api/nothing", false),
        ("/api", false),
        ("/ingest/claude-code", false),
        ("/missing.js", false),
        ("/_app/immutable/chunk.abc.css", false),
        ("/sessions/v1.2/", true), // a dot in a folder name, not in the last segment
    ];
    let actual: Vec<_> = cases.iter().map(|(path, _)| (*path, serves_fallback(path))).collect();
    assert_eq!(actual, cases.to_vec());
}
