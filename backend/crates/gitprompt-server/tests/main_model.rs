//! End to end: a Claude Code prompt (hooks) and its telemetry (OTLP) through ingest, the index and
//! the API. The model every page shows is the one that answered (main thread), even when background
//! calls on another model outnumber it. The unit rules are in gitprompt-insights/tests/main_model.rs.

mod common;

use common::{api, hook_payload, otlp, with};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const PID: &str = "550e8400-e29b-41d4-a716-446655440000";

fn api_request(model: &str, query_source: &str) -> Value {
    otlp(
        json!({"service.name": "claude-code"}),
        json!({
            "event.name": "api_request",
            "prompt.id": PID,
            "session.id": "abc123",
            "model": model,
            "query_source": query_source,
            "input_tokens": {"intValue": "100"},
            "cost_usd": {"doubleValue": 0.01},
        }),
    )
}

#[tokio::test]
async fn the_prompt_shows_the_model_that_answered_not_the_busiest_one() {
    let api = api();
    api.ingest("claude-code", hook_payload(&api.root())).await;
    api.ingest_otlp("claude-code-otel", api_request("claude-haiku-5-5", "generate_session_title")).await;
    for _ in 0..2 {
        api.ingest_otlp("claude-code-otel", api_request("claude-opus-5-5", "repl_main_thread")).await;
    }
    for _ in 0..5 {
        api.ingest_otlp("claude-code-otel", api_request("claude-haiku-5-5", "web_search_tool")).await;
    }
    api.ingest("claude-code", with(&hook_payload(&api.root()), json!({"hook_event_name": "Stop", "last_assistant_message": "Done"}))).await;

    let listed = api.json("/api/prompts").await;
    let facts = &listed[0]["facts"];
    assert_eq!((&facts["model"], &facts["model_source"]), (&json!("claude-opus-5-5"), &json!("usage")));
    let mut models: Vec<String> = serde_json::from_value(facts["models"].clone()).expect("models");
    models.sort();
    assert_eq!(models, ["claude-haiku-5-5", "claude-opus-5-5"]);
    assert_eq!(facts["api_requests"], 8);

    let id = listed[0]["id"].as_i64().expect("id");
    assert_eq!(api.json(&format!("/api/prompts/{id}")).await["facts"]["model"], "claude-opus-5-5");
}
