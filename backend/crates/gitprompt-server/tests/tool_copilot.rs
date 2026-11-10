//! GitHub Copilot CLI end to end (data only): it sends no prompt id, so prompts are named
//! `<source>#<id>` and events join the latest prompt of their session; a remap redoes the join.
//! Port of test_copilot_events_join_their_prompt_by_session_order (test_connectors.py).

mod common;

use common::{Api, api};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SESSION: &str = "8f14e45f-ceea-467f-a0e6-2c1d1a9b7e10";

fn copilot(event: &str, extra: Value) -> Value {
    let mut payload = json!({"hook_event_name": event, "session_id": SESSION, "timestamp": "2026-11-07T10:00:00.000Z", "cwd": "/work/app"});
    payload.as_object_mut().expect("object").extend(extra.as_object().expect("object").clone());
    payload
}

async fn two_turns(api: &Api) {
    api.ingest("copilot-cli", copilot("UserPromptSubmit", json!({"prompt": "Add a retry to the upload client"}))).await;
    api.ingest(
        "copilot-cli",
        copilot(
            "PostToolUse",
            json!({"tool_name": "bash", "tool_input": {"command": "pytest"}, "tool_result": {"result_type": "success"}}),
        ),
    )
    .await;
    api.ingest(
        "copilot-cli",
        copilot("PostToolUseFailure", json!({"tool_name": "edit", "tool_input": {"path": "a.py"}, "error": "File not found"})),
    )
    .await;
    api.ingest("copilot-cli", copilot("Stop", json!({"stop_reason": "end_turn", "stop_hook_active": false}))).await;
    api.ingest("copilot-cli", copilot("UserPromptSubmit", json!({"prompt": "/review the staged changes"}))).await;
    api.ingest("copilot-cli", copilot("PostToolUse", json!({"tool_name": "view", "tool_input": {"path": "a.py"}}))).await;
}

async fn yours(api: &Api) -> Vec<Value> {
    api.json_with("/api/prompts", &[("origin", "typed"), ("origin", "slash_command")]).await.as_array().expect("list").clone()
}

#[tokio::test]
async fn events_join_the_latest_prompt_of_their_session() {
    let api = api();
    two_turns(&api).await;
    let listed = yours(&api).await;
    assert_eq!(listed.len(), 2);
    let (review, first) = (&listed[0], &listed[1]);
    assert_eq!(first["fields"]["prompt_id"], json!(format!("copilot-cli#{}", first["id"])));
    assert_eq!(first["origin"], "typed");
    assert_eq!((&first["summary"]["tool_calls"], &first["summary"]["tool_failures"]), (&json!({"bash": 1, "edit": 1}), &json!(1)));
    assert_eq!(first["summary"]["ended_by"], "Stop");
    assert_eq!((&review["origin"], &review["summary"]["tool_calls"]), (&json!("slash_command"), &json!({"view": 1})));
}

#[tokio::test]
async fn a_remap_redoes_the_session_join_instead_of_losing_it() {
    let api = api();
    two_turns(&api).await;
    let before = yours(&api).await;
    assert_eq!(api.post("/api/sources/copilot-cli/remap").dashboard().send().await.status, 200);
    let after = yours(&api).await;
    let shape = |list: &[Value]| {
        list.iter().map(|p| (p["id"].clone(), p["fields"]["prompt_id"].clone(), p["summary"]["tool_calls"].clone())).collect::<Vec<_>>()
    };
    assert_eq!(shape(&after), shape(&before));
}
