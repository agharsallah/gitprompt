//! `gitprompt hook <source>`: the forwarder a tool's hook runs. It must exit 0 and print nothing
//! whether GitPrompt is up, down or hung (Claude Code shows "hook error" otherwise; Codex adds
//! stdout to the conversation); a payload that can't be delivered is spooled and delivered when
//! the daemon starts. Port of test_hook_forwarder.py's contract, for the new native forwarder.
//! Loopback only.

mod common;

use std::time::{Duration, Instant};

use common::{free_port, home, prompt_texts};
use pretty_assertions::assert_eq;
use serde_json::json;

fn prompt(text: &str) -> String {
    json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt_id": text, "prompt": text}).to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn with_the_daemon_up_the_payload_is_delivered_silently() {
    let home = home();
    let daemon = home.daemon().await;
    home.cmd_on(daemon.local_addr().port(), &["hook", "claude-code"])
        .write_stdin(prompt("delivered"))
        .assert()
        .code(0)
        .stdout("")
        .stderr("");
    assert_eq!(prompt_texts(&daemon).await, ["delivered"]);
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn with_the_daemon_down_it_exits_0_silently_and_the_payload_arrives_at_the_next_start() {
    let home = home();
    home.cmd_on(free_port(), &["hook", "claude-code"]).write_stdin(prompt("while down")).assert().code(0).stdout("").stderr("");
    let daemon = home.daemon().await; // drains the spool before it returns
    assert_eq!(prompt_texts(&daemon).await, ["while down"]);
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn spooled_payloads_are_delivered_once_and_in_order() {
    let home = home();
    let down = free_port();
    home.cmd_on(down, &["hook", "claude-code"]).write_stdin(prompt("first")).assert().code(0);
    home.cmd_on(down, &["hook", "claude-code"]).write_stdin(prompt("second")).assert().code(0);
    let daemon = home.daemon().await;
    assert_eq!(prompt_texts(&daemon).await, ["second", "first"]); // newest first
    daemon.shutdown().await.expect("shutdown");
    let again = home.daemon().await;
    assert_eq!(prompt_texts(&again).await.len(), 2); // the spool was emptied, nothing delivered twice
    again.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_that_never_answers_does_not_hold_the_tool_and_the_payload_is_spooled() {
    let home = home();
    let hung = std::net::TcpListener::bind("127.0.0.1:0").expect("bind"); // accepts connections, never answers
    let started = Instant::now();
    home.cmd_on(hung.local_addr().expect("addr").port(), &["hook", "claude-code"])
        .write_stdin(prompt("hung"))
        .assert()
        .code(0)
        .stdout("")
        .stderr("");
    assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    drop(hung);
    let daemon = home.daemon().await;
    assert_eq!(prompt_texts(&daemon).await, ["hung"]);
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_payload_the_daemon_refuses_is_not_spooled() {
    let home = home();
    let daemon = home.daemon().await;
    let port = daemon.local_addr().port();
    home.cmd_on(port, &["hook", "nope"]).write_stdin(prompt("unknown source")).assert().code(0).stdout("").stderr("");
    home.cmd_on(port, &["spool", "drain"]).assert().success().stdout("Delivered 0 spooled payloads, 0 left.\n");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn input_that_is_not_json_is_dropped_silently() {
    let home = home();
    let down = free_port();
    home.cmd_on(down, &["hook", "claude-code"]).write_stdin("not json {").assert().code(0).stdout("").stderr("");
    home.cmd_on(down, &["hook", "claude-code"]).write_stdin("").assert().code(0).stdout("").stderr("");
    let daemon = home.daemon().await;
    assert_eq!(prompt_texts(&daemon).await, Vec::<String>::new());
    daemon.shutdown().await.expect("shutdown");
}

#[test]
fn a_broken_home_still_exits_0_silently() {
    let home = home();
    std::fs::create_dir_all(home.settings.home.parent().expect("root")).expect("mkdir");
    std::fs::write(&home.settings.home, "a file where the data directory should be").expect("write");
    home.cmd_on(free_port(), &["hook", "claude-code"]).write_stdin(prompt("nowhere to go")).assert().code(0).stdout("").stderr("");
}
