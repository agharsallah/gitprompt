//! The static dashboard with its SPA fallback, and /healthz. Port of
//! test_dashboard_serves_the_fallback_page_for_story_paths (test_api.py).

mod common;

use std::path::Path;

use common::{Api, api, harness};
use gitprompt_server::dashboard::serves_fallback;
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
