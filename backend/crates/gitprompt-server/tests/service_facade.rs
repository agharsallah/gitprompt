//! The Service facade is the Tauri integration point: each method returns exactly what its HTTP
//! route answers, and its errors carry the same status and detail. A few routes, compared as JSON.
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

use common::{api, hook_payload, with};
use gitprompt_server::model::{Annotation, PromptQuery, StatsQuery};
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::GroupBy;
use pretty_assertions::assert_eq;
use serde_json::{json, to_value};

#[tokio::test]
async fn the_prompt_list_is_the_same_through_the_service_and_http() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    let direct = api.service().prompts(PromptQuery::default()).await.expect("prompts");
    assert_eq!(to_value(direct).expect("json"), api.json("/api/prompts").await);
}

#[tokio::test]
async fn a_prompt_detail_is_the_same_through_the_service_and_http() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].as_i64().expect("id");
    let direct = api.service().prompt(id).await.expect("prompt");
    assert_eq!(to_value(direct).expect("json"), api.json(&format!("/api/prompts/{id}")).await);
}

#[tokio::test]
async fn filtered_stats_are_the_same_through_the_service_and_http() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let query = StatsQuery {
        group_by: Some(GroupBy::Source),
        filter: PromptFilter { q: Some("factorial".into()), ..PromptFilter::default() },
        ..StatsQuery::default()
    };
    let direct = api.service().stats(query).await.expect("stats");
    assert_eq!(to_value(direct).expect("json"), api.json("/api/stats?group_by=source&q=factorial").await);
}

#[tokio::test]
async fn sources_and_settings_are_the_same_through_the_service_and_http() {
    let api = api();
    let service = api.service();
    assert_eq!(to_value(service.sources().await.expect("sources")).expect("json"), api.json("/api/sources").await);
    assert_eq!(to_value(service.preferences().await.expect("settings")).expect("json"), api.json("/api/settings").await);
}

#[tokio::test]
async fn an_annotation_saved_through_the_service_is_seen_over_http() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].as_i64().expect("id");
    let note = Annotation { starred: true, rating: 1, tags: vec!["release".into()], note: "good".into() };
    assert_eq!(api.service().annotate(id, note.clone()).await.expect("annotate"), note);
    assert_eq!(api.json(&format!("/api/prompts/{id}")).await["annotation"], to_value(&note).expect("json"));
}

#[tokio::test]
async fn ingest_through_the_service_publishes_its_notice() {
    let api = api();
    let mut live = api.app.bus().subscribe();
    let notice = api.service().ingest("claude-code", hook_payload(&api.root())).await.expect("ingest");
    assert_eq!((notice.prompts, notice.events), (1, 0));
    assert_eq!(live.try_next(), Some(notice));
}

#[tokio::test]
async fn service_errors_carry_the_http_status_and_detail() {
    let api = api();
    let error = api.service().prompt(999).await.expect_err("no such prompt");
    assert_eq!((error.status(), error.body().detail), (404, "prompt not found".to_string()));
    let unknown = api.service().ingest("nope", json!({})).await.expect_err("unknown source");
    assert_eq!(unknown.status(), 404);
}

#[tokio::test]
async fn the_service_needs_no_token_or_header() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let id = api.json("/api/prompts").await[0]["id"].as_i64().expect("id");
    let redacted = api.service().redact(id).await.expect("redact without X-GitPrompt");
    assert_eq!(redacted.updated, 1); // the prompt row, rewritten (its turn has no events)
}
