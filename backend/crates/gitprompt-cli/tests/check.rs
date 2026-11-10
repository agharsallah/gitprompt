//! `gitprompt check`: per connector, whether each layer is connected, and the newest prompt of
//! each app it was typed in (README "Check it live"). Port of
//! test_gitprompt_check_prints_each_connector_and_app (test_desktop_apps.py).

mod common;

use common::{home, stdout};
use serde_json::json;

fn otlp_prompt(prompt_id: &str, entrypoint: &str) -> serde_json::Value {
    let attrs = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| json!({"key": k, "value": {"stringValue": v}})).collect::<Vec<_>>();
    json!({"resourceLogs": [{
        "resource": {"attributes": attrs(&[("app.entrypoint", entrypoint)])},
        "scopeLogs": [{"logRecords": [{"attributes": attrs(&[("event.name", "user_prompt"), ("prompt.id", prompt_id)])}]}],
    }]})
}

async fn check_output() -> String {
    let home = home();
    let cwd = home.settings.user_home.join("work");
    std::fs::create_dir_all(&cwd).expect("mkdir");
    let prompt =
        json!({"hook_event_name": "UserPromptSubmit", "session_id": "s-desk", "prompt_id": "p1", "cwd": cwd.to_str(), "prompt": "task 1"});
    let app = gitprompt_server::App::new(home.settings.clone(), home.options()).expect("app");
    app.service().ingest("claude-code", prompt).await.expect("ingest");
    app.service()
        .ingest_otlp("claude-code-otel", gitprompt_server::service::Signal::Logs, otlp_prompt("p1", "claude-vscode"))
        .await
        .expect("otlp");
    drop(app);
    let output = home.cmd(&["check"]).assert().success().get_output().clone();
    stdout(&output)
}

#[tokio::test]
async fn check_reports_each_connectors_layers_and_detection() {
    let out = check_output().await;
    assert!(out.contains("Claude Code: Hooks not connected, Telemetry not connected (not detected on this machine)"), "{out}");
    assert!(out.contains("Cursor: Hooks not connected (not detected on this machine)"), "{out}");
}

#[tokio::test]
async fn check_lists_the_newest_prompt_per_app_with_what_the_rules_read() {
    let out = check_output().await;
    let line = out.lines().find(|l| l.trim_start().starts_with("VS Code")).unwrap_or_else(|| panic!("no VS Code line: {out}"));
    assert!(line.contains("app.entrypoint=claude-vscode") && line.contains("1 prompts") && line.contains("session s-desk"), "{line}");
    assert!(line.contains("work") && line.contains("no model"), "{line}");
}

#[tokio::test]
async fn check_says_when_a_connector_has_captured_nothing_and_what_it_cannot_capture() {
    let out = check_output().await;
    assert!(out.contains("no prompt captured yet"), "{out}");
    assert!(out.contains("not capturable: Claude Desktop chat"), "{out}");
}

#[test]
fn check_without_a_database_says_so_and_creates_none() {
    let home = home();
    let output = home.cmd(&["check"]).assert().success().get_output().clone();
    let expected = format!("No database yet at {}: start GitPrompt and send a prompt first.", home.db_path().display());
    assert!(stdout(&output).contains(&expected), "{}", stdout(&output));
    assert!(!home.db_path().exists());
}
