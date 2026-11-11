//! /api/stories/* and /api/search, end to end through ingest (synthetic payloads, a temp git
//! repository). Port of `test_story_routes.py`.
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

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use common::{Api, api, git, project_id, repo};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Two sessions in one repository: a failed turn with a subagent, a task notification, a retry
/// that ends well, a commit after it, and a second session the next day.
async fn data() -> (Api, PathBuf) {
    let api = api();
    let repo = repo(&api.root().join("app"), Some("2026-03-01T08:00:00+00:00"));
    let cwd = repo.to_str().expect("utf-8").to_string();
    // (event, prompt_id, session, received_at, extra)
    let sends = [
        ("UserPromptSubmit", "p1", "s1", "2026-03-02T09:00:00", json!({"prompt": "add tests for the parser"})),
        (
            "PostToolUse",
            "p1",
            "s1",
            "2026-03-02T09:00:10",
            json!({"tool_name": "Edit", "tool_input": {"file_path": format!("{cwd}/a.txt")}}),
        ),
        (
            "PostToolUse",
            "p1",
            "s1",
            "2026-03-02T09:00:20",
            json!({"tool_name": "Agent", "tool_input": {"description": "Find helpers", "subagent_type": "Explore", "prompt": "List the test helpers"}}),
        ),
        ("StopFailure", "p1", "s1", "2026-03-02T09:01:00", json!({})),
        ("UserPromptSubmit", "p2", "s1", "2026-03-02T09:02:00", json!({"prompt": "<task-notification>tests finished</task-notification>"})),
        ("UserPromptSubmit", "p3", "s1", "2026-03-02T09:03:00", json!({"prompt": "add unit tests for the parser module"})),
        ("Stop", "p3", "s1", "2026-03-02T09:04:00", json!({})),
        ("UserPromptSubmit", "p4", "s2", "2026-03-03T15:00:00", json!({"prompt": "Write the release notes for the parser"})),
        ("Stop", "p4", "s2", "2026-03-03T15:01:00", json!({})),
    ];
    let mut statements = String::new();
    let mut event_id = 0;
    for (event, pid, session, at, extra) in sends {
        let payload = common::with(&json!({"hook_event_name": event, "prompt_id": pid, "session_id": session, "cwd": cwd}), extra);
        api.ingest("claude-code", payload).await;
        let row = if event == "UserPromptSubmit" {
            format!("json_extract(fields, '$.prompt_id') = '{pid}'")
        } else {
            event_id += 1;
            format!("id = {event_id}")
        };
        let table = if event == "UserPromptSubmit" { "prompt" } else { "event" };
        write!(statements, "update {table} set received_at = '{at}.000000+00:00' where {row};").unwrap();
    }
    api.sql(&statements);
    std::fs::write(repo.join("b.txt"), "b\nc\n").expect("write");
    git(&repo, &["add", "."], None);
    git(&repo, &["commit", "-m", "Add parser tests"], Some("2026-03-02T09:03:30+00:00"));
    (api, repo)
}

async fn row_ids(api: &Api) -> BTreeMap<String, Value> {
    api.json("/api/prompts?limit=50")
        .await
        .as_array()
        .expect("list")
        .iter()
        .map(|p| (p["fields"]["prompt_id"].as_str().expect("pid").to_string(), p["id"].clone()))
        .collect()
}

#[tokio::test]
async fn a_session_story_has_chapters_with_system_markers_between_them() {
    let (api, repo) = data().await;
    let ids = row_ids(&api).await;
    let story = api.json("/api/stories/session/s1").await;
    let kinds: Vec<&Value> = story["items"].as_array().expect("items").iter().map(|i| &i["kind"]).collect();
    assert_eq!(kinds, [&json!("chapter"), &json!("markers"), &json!("chapter")]);
    let (first, markers) = (&story["items"][0], &story["items"][1]["items"]);
    assert_eq!(
        (&first["id"], &first["outcome"], &first["files"]),
        (&ids["p1"], &json!("failed"), &json!([format!("{}/a.txt", repo.display())]))
    );
    let marker_kinds: Vec<(Value, Value)> =
        markers.as_array().expect("markers").iter().map(|m| (m["kind"].clone(), m["id"].clone())).collect();
    assert_eq!(marker_kinds, [(json!("subagent_prompt"), ids["p1"].clone()), (json!("task_notification"), ids["p2"].clone())]);
    assert_eq!(markers[1]["excerpt"], "<task-notification>tests finished</task-notification>");
}

#[tokio::test]
async fn a_retry_chapter_points_to_the_one_it_rewords_and_gets_the_commit_made_after_it() {
    let (api, repo) = data().await;
    let ids = row_ids(&api).await;
    let story = api.json("/api/stories/session/s1").await;
    let retry = &story["items"][2];
    assert_eq!((&retry["retry_of"], &retry["outcome"], &retry["turn"]), (&ids["p1"], &json!("done"), &json!(3)));
    let subjects: Vec<&Value> = retry["commits"].as_array().expect("commits").iter().map(|c| &c["subject"]).collect();
    assert_eq!(subjects, [&json!("Add parser tests")]);
    assert_eq!(retry["diffstat"]["insertions"], 2);
    assert_eq!((&story["project"]["id"], &story["branches"]), (&json!(project_id(repo.to_str().expect("utf-8"))), &json!(["main"])));
    assert_eq!((&story["totals"]["chapters"], &story["totals"]["markers"]), (&json!(2), &json!(2)));
}

#[tokio::test]
async fn a_project_story_has_phases_split_by_a_long_gap() {
    let (api, repo) = data().await;
    let repo = repo.to_str().expect("utf-8");
    let story = api.json(&format!("/api/stories/project/{}", project_id(repo))).await;
    assert_eq!((&story["repo"], &story["system"]), (&json!(repo), &json!(1)));
    let phases: Vec<(Value, Value)> =
        story["phases"].as_array().expect("phases").iter().map(|p| (p["reason"].clone(), p["prompts"].clone())).collect();
    assert_eq!(phases, [(json!("start"), json!(2)), (json!("gap"), json!(1))]);
    assert_eq!((&story["totals"]["prompts"], &story["totals"]["diffstat"]["commits"]), (&json!(3), &json!(1)));
    let texts: Vec<&str> = story["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .flat_map(|p| p["items"].as_array().expect("items"))
        .map(|e| e["text"].as_str().expect("text"))
        .collect();
    assert!(texts.iter().all(|t| !t.starts_with("<task-notification>")), "{texts:?}");
}

#[tokio::test]
async fn unknown_stories_are_404() {
    let (api, _) = data().await;
    let paths = [
        "/api/stories/session/nope".to_string(),
        format!("/api/stories/project/{}", project_id("/nowhere")),
        "/api/stories/project/%25%25".to_string(),
        "/api/stories/prompt/9999".to_string(),
    ];
    let mut statuses = Vec::new();
    for path in &paths {
        statuses.push(api.get(path).send().await.status);
    }
    assert_eq!(statuses, [404, 404, 404, 404]);
}

#[tokio::test]
async fn the_lineage_lists_a_prompts_versions_with_their_diff() {
    let (api, _) = data().await;
    let ids = row_ids(&api).await;
    let lineage = api.json(&format!("/api/stories/prompt/{}", ids["p3"])).await;
    let versions: Vec<(Value, Value, Value)> = lineage["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .map(|v| (v["id"].clone(), v["place"].clone(), v["change"].clone()))
        .collect();
    assert_eq!(versions, [(ids["p1"].clone(), json!("session"), Value::Null), (ids["p3"].clone(), json!("self"), json!("improved"))]);
    let diff = lineage["versions"][1]["diff"].as_array().expect("diff");
    assert!(diff.iter().any(|op| op[0] == "insert" && op[1].as_str().is_some_and(|t| t.contains("unit"))), "{diff:?}");
}

#[tokio::test]
async fn a_task_notification_has_no_lineage() {
    let (api, _) = data().await;
    let ids = row_ids(&api).await;
    assert_eq!(api.json(&format!("/api/stories/prompt/{}", ids["p2"])).await["versions"], json!([]));
}

#[tokio::test]
async fn search_returns_snippets_with_every_match_highlighted() {
    let (api, repo) = data().await;
    let found = api.json_with("/api/search", &[("q", "PARSER tests")]).await;
    assert_eq!(found["total"], 2);
    assert_eq!(
        found["hits"][0]["snippet"],
        json!([
            {"text": "add unit ", "hit": false},
            {"text": "tests", "hit": true},
            {"text": " for the ", "hit": false},
            {"text": "parser", "hit": true},
            {"text": " module", "hit": false},
        ])
    );
    assert_eq!(found["hits"][0]["project"]["id"], json!(project_id(repo.to_str().expect("utf-8"))));
}

#[tokio::test]
async fn search_takes_the_prompt_filters_and_counts_every_page() {
    let (api, _) = data().await;
    let total = |v: Value| v["total"].clone();
    assert_eq!(
        total(api.json_with("/api/search", &[("q", "parser"), ("origin", "typed"), ("since", "2026-03-03T00:00:00+00:00")]).await),
        1
    );
    assert_eq!(total(api.json_with("/api/search", &[("q", "tests finished")]).await), 1); // every origin unless filtered
    assert_eq!(total(api.json_with("/api/search", &[("q", "tests finished"), ("origin", "typed")]).await), 0);
    let page = api.json_with("/api/search", &[("q", "parser"), ("limit", "1")]).await;
    assert_eq!((&page["total"], page["hits"].as_array().map(Vec::len)), (&json!(3), Some(1)));
}
