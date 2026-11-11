//! The security layer: Host check (DNS rebinding), the hook token on /ingest, the X-GitPrompt header
//! on writes, and the JSON error body. Port of `test_rejects_unknown_hosts`, the token and header
//! assertions spread over tests/api/*.py, plus the guard tables.
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

use common::{api, hook_payload};
use gitprompt_server::ApiError;
use gitprompt_server::security::{check_dashboard, check_token, is_local_host};
use serde_json::json;

#[tokio::test]
async fn a_request_naming_another_host_is_rejected_with_400() {
    let api = api();
    assert_eq!(api.get("/api/sources").host("evil.example").send().await.status, 400);
}

#[tokio::test]
async fn a_rebound_name_that_starts_like_a_local_host_is_rejected() {
    let api = api();
    assert_eq!(api.get("/healthz").host("127.0.0.1.evil.example").send().await.status, 400);
    assert_eq!(api.get("/healthz").host("localhost.evil.example:7878").send().await.status, 400);
}

#[tokio::test]
async fn a_request_without_a_host_header_is_rejected() {
    let api = api();
    assert_eq!(api.get("/healthz").without("host").send().await.status, 400);
}

#[tokio::test]
async fn localhost_and_loopback_are_accepted_on_any_port() {
    let api = api();
    let mut statuses = Vec::new();
    for host in ["127.0.0.1", "127.0.0.1:7878", "localhost", "localhost:5173"] {
        statuses.push((host, api.get("/healthz").host(host).send().await.status));
    }
    assert_eq!(statuses, [("127.0.0.1", 200), ("127.0.0.1:7878", 200), ("localhost", 200), ("localhost:5173", 200)]);
}

#[tokio::test]
async fn the_host_check_also_guards_ingest() {
    let api = api();
    let response = api.post("/ingest/claude-code").host("evil.example").auth().json(hook_payload(&api.root())).send().await;
    assert_eq!(response.status, 400);
    assert_eq!(api.json("/api/prompts").await, json!([]));
}

#[tokio::test]
async fn ingest_without_a_token_is_401_and_stores_nothing() {
    let api = api();
    let response = api.post("/ingest/claude-code").json(hook_payload(&api.root())).send().await;
    assert_eq!((response.status, response.json()), (401, json!({"detail": "missing or wrong token"})));
    assert_eq!(api.json("/api/prompts").await, json!([]));
}

#[tokio::test]
async fn ingest_with_a_wrong_token_is_401() {
    let api = api();
    let response = api.post("/ingest/claude-code").bearer("not-the-token").json(hook_payload(&api.root())).send().await;
    assert_eq!(response.status, 401);
}

#[tokio::test]
async fn the_token_must_be_a_bearer_credential() {
    let api = api();
    let token = api.token();
    let response = api.post("/ingest/claude-code").header("authorization", &token).json(hook_payload(&api.root())).send().await;
    assert_eq!(response.status, 401);
}

#[tokio::test]
async fn the_otlp_receiver_needs_the_token_too() {
    let api = api();
    assert_eq!(api.post("/ingest/claude-code-otel/v1/logs").json(json!({})).send().await.status, 401);
}

#[tokio::test]
async fn every_write_route_refuses_a_request_without_the_dashboard_header() {
    let api = api();
    let puts = ["/api/prompts/1/annotation", "/api/settings"];
    let posts = [
        "/api/prompts/1/redact",
        "/api/sources/claude-code/connect",
        "/api/sources/claude-code/disconnect",
        "/api/sources/claude-code/enable",
        "/api/sources/claude-code/disable",
        "/api/sources/claude-code/remap",
        "/api/data/compact",
        "/api/data/delete-events",
        "/api/security/rotate-token",
    ];
    let mut refused = Vec::new();
    for path in puts {
        let response = api.put(path).json(json!({})).send().await;
        refused.push((path, response.status, response.json()));
    }
    for path in posts {
        let response = api.post(path).json(json!({})).send().await;
        refused.push((path, response.status, response.json()));
    }
    let expected: Vec<_> = puts.iter().chain(&posts).map(|path| (*path, 403, json!({"detail": "missing X-GitPrompt header"}))).collect();
    assert_eq!(refused, expected);
}

#[tokio::test]
async fn a_dashboard_header_with_another_value_is_refused() {
    let api = api();
    let response = api.put("/api/settings").header("x-gitprompt", "0").json(json!({"week_start": "sunday"})).send().await;
    assert_eq!(response.status, 403);
    assert_eq!(api.json("/api/settings").await["settings"]["week_start"]["value"], "monday");
}

#[tokio::test]
async fn reads_need_neither_token_nor_dashboard_header() {
    let api = api();
    let paths = ["/api/prompts", "/api/sources", "/api/settings", "/api/data", "/api/security", "/api/stats", "/api/fields"];
    let mut statuses = Vec::new();
    for path in paths {
        statuses.push((path, api.get(path).send().await.status));
    }
    assert_eq!(statuses, paths.map(|p| (p, 200)));
}

#[tokio::test]
async fn domain_errors_answer_json_detail_with_their_status() {
    let api = api();
    let response = api.get("/api/prompts/999").send().await;
    assert_eq!((response.status, response.json()), (404, json!({"detail": "prompt not found"})));
}

#[test]
fn local_host_names_are_loopback_and_localhost_with_an_optional_port() {
    let cases = [
        ("127.0.0.1", true),
        ("127.0.0.1:7878", true),
        ("localhost", true),
        ("localhost:80", true),
        ("evil.example", false),
        ("127.0.0.1.evil.example", false),
        ("evil.example:127.0.0.1", false),
        ("", false),
        ("0.0.0.0", false),
    ];
    let actual: Vec<_> = cases.iter().map(|(host, _)| (*host, is_local_host(host))).collect();
    assert_eq!(actual, cases.to_vec());
}

#[test]
fn the_token_check_accepts_only_the_exact_bearer_value() {
    assert!(check_token(Some("Bearer s3cret"), "s3cret").is_ok());
    let wrong = [None, Some(""), Some("s3cret"), Some("Bearer s3cret "), Some("Bearer s3cre"), Some("bearer s3cret")];
    let accepted: Vec<_> = wrong.into_iter().filter(|h| !matches!(check_token(*h, "s3cret"), Err(ApiError::Unauthorized))).collect();
    assert_eq!(accepted, Vec::<Option<&str>>::new());
}

#[test]
fn the_dashboard_check_accepts_only_the_value_1() {
    assert!(check_dashboard(Some("1")).is_ok());
    let wrong = [None, Some(""), Some("0"), Some("true")];
    let accepted: Vec<_> = wrong.into_iter().filter(|h| !matches!(check_dashboard(*h), Err(ApiError::MissingDashboardHeader))).collect();
    assert_eq!(accepted, Vec::<Option<&str>>::new());
}

#[test]
fn api_errors_map_to_their_http_status() {
    let cases = [
        (ApiError::Unauthorized, 401),
        (ApiError::MissingDashboardHeader, 403),
        (ApiError::UntrustedHost, 400),
        (ApiError::Unprocessable("limit".into()), 422),
        (ApiError::NotFound("chart".into()), 404),
        (ApiError::Domain(gitprompt_core::Error::NotFound("prompt not found".into())), 404),
        (ApiError::Domain(gitprompt_core::Error::Conflict("off".into())), 409),
        (ApiError::Domain(gitprompt_core::Error::Invalid("bad".into())), 400),
    ];
    let actual: Vec<u16> = cases.iter().map(|(e, _)| e.status()).collect();
    assert_eq!(actual, cases.iter().map(|(_, s)| *s).collect::<Vec<_>>());
    assert_eq!(ApiError::Domain(gitprompt_core::Error::NotFound("prompt not found".into())).body().detail, "prompt not found");
}
