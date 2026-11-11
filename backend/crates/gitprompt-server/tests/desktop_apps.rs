//! Prompts typed in desktop apps and IDEs land in the right session and project, carry their
//! surface, and can be filtered, grouped and checked (ADR-0022). Real temporary git repositories,
//! synthetic payloads. Port of `test_desktop_apps.py` (the store-level `latest` test belongs to the
//! store crate; `gitprompt check` is in the CLI tests).
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

use std::path::{Path, PathBuf};

use common::{Api, api, git, otlp, repo};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A repository `shop` with one commit, a `pkg` folder and a linked worktree on branch `feature`.
fn shop(api: &Api) -> PathBuf {
    let main = repo(&api.root().join("shop"), None);
    std::fs::create_dir_all(main.join("pkg")).expect("mkdir");
    let worktree = main.join(".claude/worktrees/feature");
    git(&main, &["worktree", "add", "-b", "feature", worktree.to_str().expect("utf-8")], None);
    main
}

fn claude_prompt(cwd: &Path, n: u32, session: &str) -> Value {
    json!({"hook_event_name": "UserPromptSubmit", "session_id": session, "prompt_id": format!("p{n}"), "cwd": cwd.to_str(), "prompt": format!("task {n}")})
}

/// A telemetry `user_prompt` record whose resource carries `resource`.
fn telemetry(prompt_id: &str, resource: Value) -> Value {
    otlp(resource, json!({"event.name": "user_prompt", "prompt.id": prompt_id}))
}

fn cursor_prompt(roots: &[&str]) -> Value {
    json!({
        "hook_event_name": "beforeSubmitPrompt", "conversation_id": "conv-1", "generation_id": "g1", "workspace_roots": roots,
        "model": "gpt-6", "cursor_version": "3.4.1", "user_email": "someone@example.test", "prompt": "rename the cart",
    })
}

fn sorted_by_id(list: Value) -> Vec<Value> {
    let mut list = list.as_array().expect("list").clone();
    list.sort_by_key(|p| p["id"].as_i64());
    list
}

#[tokio::test]
async fn a_worktree_prompt_belongs_to_the_main_repository() {
    let api = api();
    let main = shop(&api);
    let worktree = main.join(".claude/worktrees/feature");
    api.ingest("claude-code", claude_prompt(&worktree, 1, "s-desk")).await;
    api.ingest("claude-code", claude_prompt(&main.join("pkg"), 2, "s-desk")).await; // a subfolder of the main checkout
    let prompts = sorted_by_id(api.json("/api/prompts").await);
    let (first, second) = (&prompts[0], &prompts[1]);
    let root = json!(main.to_str());
    assert_eq!((&first["context"]["git_root"], &second["context"]["git_root"]), (&root, &root));
    assert_eq!((&first["context"]["git_worktree_root"], &first["context"]["git_branch"]), (&json!(worktree.to_str()), &json!("feature")));
    assert!(second["context"].get("git_worktree_root").is_none());
    let repos: Vec<Value> =
        api.json("/api/stats?group_by=repo").await["groups"].as_array().expect("groups").iter().map(|g| g["key"].clone()).collect();
    assert_eq!(repos, [root]);
}

#[tokio::test]
async fn a_folder_without_git_is_its_own_project() {
    let api = api();
    let notes = api.root().join("notes");
    std::fs::create_dir_all(&notes).expect("mkdir");
    api.ingest("claude-code", claude_prompt(&notes, 1, "s-desk")).await;
    assert!(api.json("/api/prompts").await[0]["context"].get("git_root").is_none());
    assert_eq!(api.json("/api/stats?group_by=repo").await["groups"][0]["key"], json!(notes.to_str()));
}

#[tokio::test]
async fn a_multi_root_cursor_workspace_uses_its_first_root() {
    let api = api();
    let main = shop(&api);
    let pkg = main.join("pkg");
    let other = api.root();
    api.ingest("cursor", cursor_prompt(&[pkg.to_str().expect("utf-8"), other.to_str().expect("utf-8")])).await;
    let prompt = api.json("/api/prompts").await[0].clone();
    assert_eq!(prompt["context"]["git_root"], json!(main.to_str()));
    assert_eq!(prompt["fields"]["workspace_roots"], json!([pkg.to_str(), other.to_str()]));
    assert_eq!((&prompt["fields"]["session_id"], &prompt["facts"]["connector"]["label"]), (&json!("conv-1"), &json!("Cursor")));
    assert_eq!(prompt["facts"]["surface"], Value::Null); // Cursor doesn't say whether the IDE or the CLI sent it
    assert_eq!(prompt["facts"]["tool_version"], "3.4.1"); // the version Cursor reported with the prompt
    let raw = &api.json(&format!("/api/prompts/{}", prompt["id"])).await["raw"];
    assert!(raw.get("user_email").is_none(), "{raw}");
}

async fn four_surfaces(api: &Api) {
    let cwd = api.root();
    api.ingest("claude-code", claude_prompt(&cwd, 1, "s1")).await;
    api.ingest_otlp("claude-code-otel", telemetry("p1", json!({"app.entrypoint": "cli"}))).await;
    api.ingest("claude-code", claude_prompt(&cwd, 2, "s2")).await;
    api.ingest_otlp("claude-code-otel", telemetry("p2", json!({"app.entrypoint": "claude-vscode"}))).await;
    api.ingest("claude-code", claude_prompt(&cwd, 3, "s3")).await;
    api.ingest("claude-code", claude_prompt(&cwd, 4, "s4")).await;
    api.ingest_otlp(
        "claude-code-otel",
        telemetry("p4", json!({"service.name": "claude-code-desktop", "app.entrypoint": "claude-desktop"})),
    )
    .await;
}

#[tokio::test]
async fn the_surface_is_a_fact_of_each_prompt() {
    let api = api();
    four_surfaces(&api).await;
    let surfaces: std::collections::BTreeMap<String, Value> = api
        .json("/api/prompts")
        .await
        .as_array()
        .expect("list")
        .iter()
        .map(|p| (p["typed_text"].as_str().expect("text").to_string(), p["facts"]["surface"].clone()))
        .collect();
    assert_eq!(
        (&surfaces["task 1"]["id"], &surfaces["task 2"]["name"], &surfaces["task 3"]),
        (&json!("cli"), &json!("VS Code"), &Value::Null)
    );
    assert_eq!(surfaces["task 4"], json!({"id": "desktop", "name": "Desktop app", "icon": "app-window"}));
}

#[tokio::test]
async fn the_surface_is_a_filter_a_grouping_and_a_story_fact() {
    let api = api();
    four_surfaces(&api).await;
    let filtered: Vec<Value> = api
        .json_with("/api/prompts", &[("surface", "desktop"), ("surface", "cli")])
        .await
        .as_array()
        .expect("list")
        .iter()
        .map(|p| p["typed_text"].clone())
        .collect();
    assert_eq!(filtered, [json!("task 4"), json!("task 1")]);
    let groups: std::collections::BTreeMap<String, (f64, Value)> = api.json("/api/stats?group_by=surface").await["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .map(|g| (common::key(&g["key"]), (common::num(&g["prompts"]), g["detail"].clone())))
        .collect();
    let expected = [("cli", "CLI"), ("vscode", "VS Code"), ("desktop", "Desktop app")]
        .map(|(k, d)| (k.to_string(), (1.0, json!(d))))
        .into_iter()
        .chain([("null".to_string(), (1.0, Value::Null))])
        .collect();
    assert_eq!(groups, expected);
    assert_eq!(api.json("/api/stories/session/s4").await["surface"]["id"], "desktop");
}

#[tokio::test]
async fn recent_captures_show_the_newest_prompt_per_connector_and_surface() {
    let api = api();
    let main = shop(&api);
    api.ingest("claude-code", claude_prompt(&main, 1, "old")).await;
    api.ingest_otlp("claude-code-otel", telemetry("p1", json!({"app.entrypoint": "cli"}))).await;
    api.ingest("claude-code", claude_prompt(&main.join(".claude/worktrees/feature"), 2, "new")).await;
    api.ingest_otlp("claude-code-otel", telemetry("p2", json!({"app.entrypoint": "cli"}))).await;
    api.ingest("cursor", cursor_prompt(&[main.to_str().expect("utf-8")])).await;

    let rows = api.json("/api/sources/recent").await;
    let pairs: Vec<(Value, Value)> =
        rows.as_array().expect("rows").iter().map(|r| (r["connector"]["id"].clone(), r["surface"]["id"].clone())).collect();
    assert_eq!(pairs, [(json!("cursor"), Value::Null), (json!("claude-code"), json!("cli"))]);
    let claude = &rows[1];
    assert_eq!((&claude["session"], &claude["prompts"], &claude["branch"]), (&json!("new"), &json!(2), &json!("feature")));
    assert_eq!(claude["project"], json!({"repo": main.to_str(), "name": "shop"}));
    assert!(claude["worktree"].as_str().expect("worktree").ends_with("feature"));
    assert_eq!((&claude["signals"], &rows[0]["model"]), (&json!({"app.entrypoint": "cli"}), &json!("gpt-6")));
}
