//! Recent captures (captures.rs): the newest prompt of each connector and surface, where GitPrompt
//! put it and what the surface rules read. Port of the `recent`/`latest` parts of
//! tests/api/test_desktop_apps.py on an in-memory store (no git, no HTTP).

mod common;

use std::sync::Arc;

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::{obj, specs, surface_ref, utc};
use gitprompt_insights::captures::{CaptureProject, recent};
use gitprompt_insights::indexer::{CODE_VERSION, Indexer};
use gitprompt_store::Store;

fn add_prompt(store: &Store, source: &str, fields: Value, context: Value) -> i64 {
    let raw = json!({"prompt": fields["prompt"].clone()});
    store.add_prompt(source, &obj(fields), &obj(context), &raw).expect("stored").expect("not a duplicate").id
}

fn indexed(store: &Store) {
    Indexer::with(specs(), Arc::new(utc()), CODE_VERSION).ensure(store).expect("indexed");
}

#[test]
fn the_newest_prompt_of_each_connector_comes_first_with_its_prompt_count() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(&store, "codex", json!({"prompt_id": "t0", "session_id": "s0", "event": "UserPromptSubmit", "prompt": "x"}), json!({}));
    add_prompt(&store, "codex", json!({"prompt_id": "t1", "session_id": "s1", "event": "UserPromptSubmit", "prompt": "x"}), json!({}));
    add_prompt(
        &store,
        "copilot-cli",
        json!({"prompt_id": "t2", "session_id": "s2", "event": "UserPromptSubmit", "prompt": "x"}),
        json!({}),
    );
    indexed(&store);
    let rows = recent(&store, &specs()).unwrap();
    let seen: Vec<(String, i64, Option<String>)> = rows.iter().map(|r| (r.connector.id.clone(), r.prompts, r.session.clone())).collect();
    assert_eq!(seen, vec![("copilot-cli".to_string(), 1, Some("s2".to_string())), ("codex".to_string(), 2, Some("s1".to_string()))]);
    assert_eq!(rows.into_iter().map(|r| r.signals).collect::<Vec<_>>(), vec![IndexMap::<String, Value>::new(), IndexMap::new()]);
}

#[test]
fn a_capture_shows_its_surface_project_worktree_branch_and_the_signals_read() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(
        &store,
        "claude-code",
        json!({"prompt": "task 1", "prompt_id": "p1", "session_id": "new", "cwd": "/w/shop/.claude/worktrees/feature"}),
        json!({"git_root": "/w/shop", "git_branch": "feature", "git_worktree_root": "/w/shop/.claude/worktrees/feature"}),
    );
    let telemetry = json!({"event.name": "user_prompt", "prompt.id": "p1", "app.entrypoint": "cli"});
    store.add_event("claude-code-otel", &obj(json!({"event": "user_prompt", "prompt_id": "p1", "session_id": "new"})), &telemetry).unwrap();
    indexed(&store);
    let [capture] = &recent(&store, &specs()).unwrap()[..] else { panic!("one capture expected") };
    assert_eq!(
        (capture.surface.clone(), capture.project.clone()),
        (Some(surface_ref("cli", "CLI", "terminal")), Some(CaptureProject { repo: "/w/shop".into(), name: "shop".into() }))
    );
    assert_eq!(
        (capture.worktree.as_deref(), capture.branch.as_deref(), capture.session.as_deref()),
        (Some("/w/shop/.claude/worktrees/feature"), Some("feature"), Some("new"))
    );
    assert_eq!(capture.signals.clone().into_iter().collect::<Vec<_>>(), vec![("app.entrypoint".to_string(), json!("cli"))]);
    assert_eq!((capture.origin.as_str(), capture.model.clone(), capture.prompts), ("typed", None, 1));
}

#[test]
fn a_capture_project_is_named_by_its_remote_when_known() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(
        &store,
        "cursor",
        json!({"prompt": "x", "prompt_id": "g1", "session_id": "c1", "model": "gpt-6"}),
        json!({"git_root": "/w/shop", "git_repo": "acme/shop"}),
    );
    indexed(&store);
    let [capture] = &recent(&store, &specs()).unwrap()[..] else { panic!("one capture expected") };
    assert_eq!(
        (capture.project.clone(), capture.model.as_deref(), capture.surface.clone()),
        (Some(CaptureProject { repo: "/w/shop".into(), name: "acme/shop".into() }), Some("gpt-6"), None)
    );
}

#[test]
fn a_capture_without_a_folder_has_no_project() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(&store, "codex", json!({"prompt": "x", "prompt_id": "t1", "session_id": "s1"}), json!({}));
    indexed(&store);
    assert_eq!(recent(&store, &specs()).unwrap()[0].project, None);
}

#[test]
fn a_source_without_a_spec_is_named_by_its_id_with_no_surface_or_signals() {
    let store = Store::open_in_memory().unwrap();
    add_prompt(&store, "mystery", json!({"prompt": "x", "prompt_id": "m1", "cursor_version": "1"}), json!({}));
    indexed(&store);
    let [capture] = &recent(&store, &specs()).unwrap()[..] else { panic!("one capture expected") };
    assert_eq!((capture.connector.label.as_str(), capture.surface.clone(), capture.signals.is_empty()), ("mystery", None, true));
}

#[test]
fn an_empty_store_has_no_captures() {
    let store = Store::open_in_memory().unwrap();
    indexed(&store);
    assert_eq!(recent(&store, &specs()).unwrap(), vec![]);
}
