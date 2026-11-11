//! `GET /api/status` and `Service::status`: the machine-readable core of `gitprompt doctor`.
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

use common::{harness, hook_payload};
use gitprompt_capture::spool::Spool;
use gitprompt_server::Dashboard;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn now() -> jiff::Timestamp {
    "2026-11-10T10:00:00Z".parse().expect("time")
}

#[tokio::test]
async fn a_fresh_install_reports_its_version_paths_and_empty_store() {
    let api = harness().clock(now()).start();
    let status = api.json("/api/status").await;
    let home = api.settings.home.display().to_string();
    assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(status["backend"], "rust");
    assert_eq!(status["started_at"], "2026-11-10T10:00:00.000000+00:00");
    assert_eq!(status["uptime_s"], 0);
    assert_eq!(status["home"], home);
    assert_eq!(status["db_path"], api.settings.db_path().display().to_string());
    assert!(status["db_bytes"].as_u64().expect("bytes") > 0);
    assert_eq!((status["prompts"].clone(), status["events"].clone()), (json!(0), json!(0)));
    assert_eq!(status["dashboard"], json!({"source": "none", "dir": null, "built": false}));
    assert_eq!(status["spool"], json!([]));
}

#[tokio::test]
async fn the_index_is_stale_until_the_first_read_then_current() {
    let api = harness().start();
    let before = api.json("/api/status").await["index"].clone();
    assert_eq!((before["rules_version"].clone(), before["stale"].clone()), (Value::Null, json!(true)));
    api.json("/api/prompts").await; // every read brings the index up to date
    let after = api.json("/api/status").await["index"].clone();
    assert_eq!(after["rules_version"], after["current_version"]);
    assert_eq!((after["stale"].clone(), after["unindexed"].clone(), after["dirty_turns"].clone()), (json!(false), json!(0), json!(0)));
}

#[tokio::test]
async fn captured_rows_are_counted_and_new_prompts_show_as_unindexed() {
    let api = harness().start();
    api.json("/api/prompts").await;
    let payload = hook_payload(&api.root());
    api.ingest("claude-code", payload).await;
    let status = api.json("/api/status").await;
    assert_eq!(status["prompts"], 1);
    assert_eq!(status["index"]["unindexed"], 1);
    api.json("/api/prompts").await;
    assert_eq!(api.json("/api/status").await["index"]["unindexed"], 0);
}

#[tokio::test]
async fn the_spool_backlog_is_listed_per_source() {
    let api = harness().start();
    let spool = Spool::of(&api.settings);
    spool.append("claude-code", &json!({"prompt": "a"})).expect("spool");
    spool.append("claude-code", &json!({"prompt": "b"})).expect("spool");
    spool.append("codex", &json!({"prompt": "c"})).expect("spool");
    let status = api.json("/api/status").await;
    assert_eq!(status["spool"], json!([{"source": "claude-code", "payloads": 2}, {"source": "codex", "payloads": 1}]));
}

#[tokio::test]
async fn the_dashboard_source_is_reported() {
    let h = harness();
    let dir = h.root().join("build");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("index.html"), "home").expect("write");
    let api = h.dashboard(dir.clone()).start();
    let dashboard = api.json("/api/status").await["dashboard"].clone();
    assert_eq!(dashboard, json!({"source": "dir", "dir": dir.display().to_string(), "built": true}));

    let mut h = harness();
    h.options.dashboard = Some(Dashboard::Embedded);
    let dashboard = h.start().json("/api/status").await["dashboard"].clone();
    assert_eq!(dashboard["source"], "embedded");
    assert_eq!(dashboard["built"], gitprompt_dashboard::is_built());
}

#[tokio::test]
async fn status_never_shows_the_token() {
    let api = harness().start();
    let token = api.token();
    let text = api.get("/api/status").send().await.text();
    assert!(!text.contains(&token));
    assert!(!text.contains("token"), "{text}");
}

#[tokio::test]
async fn the_service_answers_what_the_route_answers() {
    let api = harness().clock(now()).start();
    let direct = serde_json::to_value(api.service().status().await.expect("status")).expect("json");
    assert_eq!(api.json("/api/status").await, direct);
}

#[tokio::test]
async fn the_health_section_reports_routes_and_the_database_writer() {
    let api = harness().start();
    api.json("/api/prompts").await;
    api.json("/api/prompts").await;
    let health = api.json("/api/status").await["health"].clone();
    let prompts = health["routes"].as_array().unwrap().iter().find(|r| r["route"] == "GET /api/prompts").expect("recorded").clone();
    assert_eq!((prompts["count"].clone(), prompts["errors"].clone(), prompts["slow"].clone()), (json!(2), json!(0), json!(0)));
    assert!(health["writer"]["entries"].as_u64().unwrap() > 0, "the first read built the index");
    assert_eq!(health["process"]["rss_bytes"], Value::Null, "sampled by the daemon only");
    assert_eq!(health["warnings"], json!([]));
}
