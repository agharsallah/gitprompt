//! POST /ingest/{source} and /ingest/{source}/v1/{signal}: hooks are never blocked (an empty 204
//! whatever was stored), OTLP gets `{}`, unknown sources are 404, a disabled source is discarded,
//! and every capture is announced on the SSE stream. Plus the connector conformance fixtures
//! replayed through HTTP and read back through the API.
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

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use common::{Api, api, harness, hook_payload, num, otlp, with};
use gitprompt_server::model::{Notification, NotificationKind, Target};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn a_hook_call_is_answered_with_an_empty_204() {
    let api = api();
    let response = api.post("/ingest/claude-code").auth().json(hook_payload(&api.root())).send().await;
    assert_eq!((response.status, response.body.len()), (204, 0));
}

#[tokio::test]
async fn the_same_prompt_id_sent_twice_is_stored_once() {
    let api = api();
    let payload = hook_payload(&api.root());
    api.ingest("claude-code", payload.clone()).await;
    api.ingest("claude-code", payload).await;
    assert_eq!(api.json("/api/prompts").await.as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn an_otlp_export_is_answered_with_an_empty_object() {
    let api = api();
    let export = otlp(json!({"service.name": "claude-code"}), json!({"event.name": "api_request", "prompt.id": "p1", "session.id": "s1"}));
    let response = api.post("/ingest/claude-code-otel/v1/logs").auth().json(export).send().await;
    assert_eq!((response.status, response.json()), (200, json!({})));
    let metrics = api.post("/ingest/claude-code-otel/v1/metrics").auth().json(json!({"resourceMetrics": []})).send().await;
    assert_eq!((metrics.status, metrics.json()), (200, json!({})));
}

#[tokio::test]
async fn the_otlp_endpoint_refuses_a_hooks_source() {
    let api = api();
    let response = api.post("/ingest/claude-code/v1/logs").auth().json(json!({})).send().await;
    assert_eq!(response.status, 404);
}

#[tokio::test]
async fn an_otlp_signal_other_than_logs_or_metrics_is_refused() {
    let api = api();
    assert_eq!(api.post("/ingest/claude-code-otel/v1/traces").auth().json(json!({})).send().await.status, 422);
}

#[tokio::test]
async fn an_unknown_source_is_404_naming_it() {
    let api = api();
    let response = api.post("/ingest/nope").auth().json(json!({"hook_event_name": "Stop"})).send().await;
    assert_eq!((response.status, response.json()), (404, json!({"detail": "unknown source 'nope'"})));
}

#[tokio::test]
async fn an_event_the_spec_drops_is_still_answered_normally_but_not_stored() {
    let api = api();
    let noise = otlp(json!({}), json!({"event.name": "hook_registered", "session.id": "s1"}));
    api.ingest_otlp("claude-code-otel", noise).await;
    assert_eq!(api.json("/api/data").await["events"], json!([]));
}

#[tokio::test]
async fn a_source_disabled_by_env_discards_payloads_with_the_usual_204() {
    let api = harness().env("GITPROMPT_DISABLED_SOURCES", "claude-code").start();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    assert_eq!(api.json("/api/prompts").await, json!([]));
}

#[tokio::test]
async fn prompts_carry_the_tool_version_observed_at_ingest() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    assert_eq!(api.json("/api/prompts").await[0]["context"]["tool_version"], common::TOOL_VERSION);
}

// --- live updates ---

/// An open SSE stream; the subscription exists once the response is returned.
struct Stream {
    status: u16,
    content_type: Option<String>,
    body: Body,
    buffer: String,
    /// The `event:` name of the last event read (`None`: a plain `data:` notice).
    last_event: Option<String>,
}

async fn open_stream(api: &Api) -> Stream {
    let request = Request::get("/api/prompts/stream").header("host", common::HOST).body(Body::empty()).expect("request");
    let response = api.router.clone().oneshot(request).await.expect("router");
    let content_type = response.headers().get("content-type").map(|v| v.to_str().expect("ascii").to_string());
    Stream { status: response.status().as_u16(), content_type, body: response.into_body(), buffer: String::new(), last_event: None }
}

impl Stream {
    /// The JSON of the next `data:` event (comments and keep-alives skipped), within 5 s.
    async fn next_notice(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(5), self.read_notice()).await.expect("a notice within 5 s")
    }

    /// The next `data:` event, reading frames until one is complete.
    async fn read_notice(&mut self) -> Value {
        loop {
            if let Some(notice) = self.take_buffered_notice() {
                return notice;
            }
            self.read_frame().await;
        }
    }

    /// The first complete `data:` event already buffered, consuming it and anything before it.
    fn take_buffered_notice(&mut self) -> Option<Value> {
        while let Some(end) = self.buffer.find("\n\n") {
            let event: String = self.buffer.drain(..end + 2).collect();
            if let Some(data) = event.lines().find_map(|l| l.strip_prefix("data:")) {
                self.last_event = event.lines().find_map(|l| l.strip_prefix("event:")).map(|name| name.trim().to_owned());
                return Some(serde_json::from_str(data.trim()).expect("notice is JSON"));
            }
        }
        None
    }

    /// Append the next body frame's text to the buffer (waiting for it).
    async fn read_frame(&mut self) {
        let frame = self.body.frame().await.expect("stream open").expect("frame");
        if let Some(bytes) = frame.data_ref() {
            self.buffer.push_str(std::str::from_utf8(bytes).expect("utf-8"));
        }
    }
}

#[tokio::test]
async fn the_stream_announces_each_capture_with_its_counts() {
    let api = api();
    let mut stream = open_stream(&api).await;
    assert_eq!(stream.status, 200);
    assert!(stream.content_type.as_deref().is_some_and(|c| c.starts_with("text/event-stream")), "{:?}", stream.content_type);

    api.ingest("claude-code", hook_payload(&api.root())).await;
    assert_eq!(stream.next_notice().await, json!({"prompts": 1, "events": 0}));
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    assert_eq!(stream.next_notice().await, json!({"prompts": 0, "events": 1}));
}

#[tokio::test]
async fn a_notification_is_a_named_event_between_the_notices() {
    let api = api();
    let mut stream = open_stream(&api).await;
    let notification =
        Notification { kind: NotificationKind::PossibleLeak, title: "A possible leak".into(), body: "b".into(), target: Target::Leaks };
    api.app.bus().notify(notification);
    api.ingest("claude-code", hook_payload(&api.root())).await;
    let expected = json!({"kind": "possible_leak", "title": "A possible leak", "body": "b", "target": {"page": "leaks"}});
    assert_eq!((stream.next_notice().await, stream.last_event.as_deref()), (expected, Some("notification")));
    assert_eq!((stream.next_notice().await, stream.last_event.as_deref()), (json!({"prompts": 1, "events": 0}), None));
}

#[tokio::test]
async fn nothing_is_announced_for_a_payload_that_stored_nothing() {
    let api = api();
    let mut stream = open_stream(&api).await;
    api.ingest("claude-code", hook_payload(&api.root())).await;
    api.ingest("claude-code", hook_payload(&api.root())).await; // a duplicate: nothing stored
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    assert_eq!(stream.next_notice().await, json!({"prompts": 1, "events": 0}));
    assert_eq!(stream.next_notice().await, json!({"prompts": 0, "events": 1})); // not the duplicate's
}

#[tokio::test]
async fn a_disabled_source_announces_nothing() {
    let api = harness().env("GITPROMPT_DISABLED_SOURCES", "copilot-cli").start();
    let mut stream = open_stream(&api).await;
    let discarded = json!({"hook_event_name": "UserPromptSubmit", "session_id": "s", "prompt": "hi"});
    api.ingest("copilot-cli", discarded).await;
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop"}))).await;
    assert_eq!(stream.next_notice().await, json!({"prompts": 0, "events": 1}));
}

// --- conformance fixtures, replayed through HTTP (testdata/fixtures/<tool>/*.json) ---

fn fixtures(tool: &str) -> Vec<(String, Value)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/fixtures").join(tool);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).expect("fixtures dir").map(|e| e.expect("entry").path()).collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let stem = p.file_stem().expect("stem").to_string_lossy().into_owned();
            (stem, serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("fixture JSON"))
        })
        .collect()
}

fn is_subset(expected: &Value, actual: &Value) -> bool {
    expected.as_object().expect("object").iter().all(|(k, v)| actual.get(k) == Some(v))
}

/// An event fixture: its position among all of the tool's fixtures, its name, its content.
type EventCase<'a> = (usize, &'a String, &'a Value);

fn stored_as<'a>(cases: &'a [(String, Value)], kind: &'a str) -> impl Iterator<Item = (usize, &'a (String, Value))> + 'a {
    cases.iter().enumerate().filter(move |(_, (_, c))| c["expect"]["stored_as"] == kind)
}

/// Replay a tool's fixtures in file-name order, then read everything back through the API.
/// Returns every mismatch (empty = the connector conforms end to end).
async fn conformance(tool: &str) -> Vec<String> {
    let api = api();
    let cases = fixtures(tool);
    for (_, case) in &cases {
        api.ingest(tool, case["payload"].clone()).await;
    }
    let mut problems = Vec::new();
    problems.extend(stored_events_problem(&api, tool, &cases).await);

    let mut listed = api.json_with("/api/prompts", &[("source", tool), ("limit", "200")]).await.as_array().expect("list").clone();
    listed.reverse(); // oldest first, like the fixtures
    let prompt_cases: Vec<(usize, &(String, Value))> = stored_as(&cases, "prompt").collect();
    if listed.len() != prompt_cases.len() {
        problems.push(format!("{} prompts expected, {} listed", prompt_cases.len(), listed.len()));
    }
    // Event rows are stored in fixture order, so the n-th event row (by id) is the n-th event case.
    let event_cases: Vec<EventCase<'_>> = stored_as(&cases, "event").map(|(i, (n, c))| (i, n, c)).collect();
    let mut joined = 0;
    for ((position, (name, case)), prompt) in prompt_cases.into_iter().zip(&listed) {
        problems.extend(prompt_problems(name, &case["expect"], prompt));
        let detail = api.json(&format!("/api/prompts/{}", prompt["id"])).await;
        joined += joined_events(name, position, &detail, &event_cases, &mut problems);
    }
    let expected_joined = cases.iter().filter(|(_, c)| c["expect"]["turn"].is_string()).count();
    if joined != expected_joined {
        problems.push(format!("{expected_joined} events should join a prompt, {joined} did"));
    }
    problems
}

/// A mismatch between the event rows stored for `tool` and its event fixtures.
async fn stored_events_problem(api: &Api, tool: &str, cases: &[(String, Value)]) -> Option<String> {
    let data = api.json("/api/data").await;
    let stored: f64 = data["events"].as_array().expect("events").iter().filter(|s| s["source"] == tool).map(|s| num(&s["rows"])).sum();
    let expected = stored_as(cases, "event").count();
    (stored as usize != expected).then(|| format!("{expected} events expected, {stored} stored"))
}

/// How the listed `prompt` differs from what fixture `name` expects of it.
fn prompt_problems(name: &str, expect: &Value, prompt: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    if !is_subset(&expect["fields"], &prompt["fields"]) {
        problems.push(format!("{name}: fields {}", prompt["fields"]));
    }
    if prompt["fields"]["prompt_id"].as_str().is_none_or(str::is_empty) {
        problems.push(format!("{name}: no prompt id"));
    }
    if prompt["origin"] != expect["origin"] {
        problems.push(format!("{name}: origin {}", prompt["origin"]));
    }
    if expect.get("typed_text").is_some() && prompt["typed_text"] != expect["typed_text"] {
        problems.push(format!("{name}: typed_text {}", prompt["typed_text"]));
    }
    problems
}

/// How many of the events joined to prompt `name` (fixture number `position`) its event fixtures
/// expect there; every other join is recorded in `problems`.
fn joined_events(name: &str, position: usize, detail: &Value, event_cases: &[EventCase<'_>], problems: &mut Vec<String>) -> usize {
    let mut joined = 0;
    for event in detail["events"].as_array().expect("events") {
        let row = event["id"].as_u64().expect("event id") as usize;
        let Some(&(at, event_name, event_case)) = row.checked_sub(1).and_then(|i| event_cases.get(i)) else {
            problems.push(format!("{name}: joined an unknown event row {row}"));
            continue;
        };
        let turn = &event_case["expect"]["turn"];
        if turn == name {
            joined += 1;
            if !is_subset(&event_case["expect"]["fields"], &event["fields"]) {
                problems.push(format!("{event_name}: fields {}", event["fields"]));
            }
        } else if !(turn.is_null() && at < position) {
            // `turn: null` means no prompt had its id when it arrived (the Python kit's check);
            // a session start sent before its prompt, with that prompt's id, joins it later.
            problems.push(format!("{event_name}: joined {name}, expected {turn}"));
        }
    }
    joined
}

#[tokio::test]
async fn claude_code_fixtures_round_trip_through_the_api() {
    assert_eq!(conformance("claude-code").await, Vec::<String>::new());
}

#[tokio::test]
async fn codex_fixtures_round_trip_through_the_api() {
    assert_eq!(conformance("codex").await, Vec::<String>::new());
}

#[tokio::test]
async fn copilot_cli_fixtures_round_trip_through_the_api() {
    assert_eq!(conformance("copilot-cli").await, Vec::<String>::new());
}

#[tokio::test]
async fn cursor_fixtures_round_trip_through_the_api() {
    assert_eq!(conformance("cursor").await, Vec::<String>::new());
}
