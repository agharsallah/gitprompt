//! `gitprompt spool drain`: deliver what the forwarder spooled to the running daemon (a daemon
//! started later drains it by itself; this is for one that was unreachable for a while).
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

use common::{free_port, home, prompt_texts};
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::json;

fn prompt(text: &str) -> String {
    json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt_id": text, "prompt": text}).to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn drain_delivers_spooled_payloads_to_the_running_daemon() {
    let home = home();
    let daemon = home.daemon().await;
    let up = daemon.local_addr().port();
    home.cmd_on(free_port(), &["hook", "claude-code"]).write_stdin(prompt("missed")).assert().code(0); // it couldn't reach the daemon
    assert_eq!(prompt_texts(&daemon).await, Vec::<String>::new());

    home.cmd_on(up, &["spool", "drain"]).assert().success().stdout("Delivered 1 spooled payloads, 0 left.\n");
    assert_eq!(prompt_texts(&daemon).await, ["missed"]);
    home.cmd_on(up, &["spool", "drain"]).assert().success().stdout("Delivered 0 spooled payloads, 0 left.\n");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn drain_with_the_daemon_down_fails_and_keeps_the_spool() {
    let home = home();
    let down = free_port();
    home.cmd_on(down, &["hook", "claude-code"]).write_stdin(prompt("kept")).assert().code(0);
    let receiver = format!("http://127.0.0.1:{down}");
    home.cmd_on(down, &["spool", "drain"])
        .assert()
        .code(1)
        .stderr(contains(format!("GitPrompt is not running on {receiver}: 1 spooled payloads left.")));
    let daemon = home.daemon().await;
    assert_eq!(prompt_texts(&daemon).await, ["kept"]);
    daemon.shutdown().await.expect("shutdown");
}
