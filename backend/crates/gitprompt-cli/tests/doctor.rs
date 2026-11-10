//! `gitprompt doctor [--json]` on temp homes: what it shows, the problems it finds, the exit code,
//! and that it changes nothing.

mod common;

use std::fs::File;
use std::io::Write;

use common::{free_port, home, stdout};
use gitprompt_capture::spool::Spool;
use gitprompt_server::App;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The JSON report, and the exit code.
fn report(home: &common::Home, port: u16) -> (Value, i32) {
    let output = home.cmd_on(port, &["doctor", "--json"]).output().expect("run");
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", stdout(&output)));
    (report, output.status.code().expect("exit code"))
}

fn problems(report: &Value) -> Vec<String> {
    report["problems"].as_array().expect("problems").iter().map(|p| p["what"].as_str().expect("what").to_owned()).collect()
}

#[test]
fn an_empty_home_is_healthy_and_left_untouched() {
    let home = home();
    let port = free_port();
    home.cmd_on(port, &["doctor"])
        .assert()
        .code(0)
        .stdout(contains("GitPrompt doctor: version"))
        .stdout(contains(format!("{port} (nothing listening)")))
        .stdout(contains("(missing: created at first start)"))
        .stdout(contains("Claude Code (claude-code):"))
        .stdout(contains("GitPrompt is not running."))
        .stdout(contains("No problems found."));
    assert!(!home.settings.home.exists(), "doctor created the data directory");
}

#[test]
fn the_json_report_of_an_empty_home() {
    let home = home();
    let port = free_port();
    let (report, code) = report(&home, port);
    assert_eq!(code, 0);
    assert_eq!((report["healthy"].clone(), report["problems"].clone()), (json!(true), json!([])));
    assert_eq!(report["backend"], "rust");
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["data"]["home"], home.settings.home.display().to_string());
    assert_eq!((report["data"]["db_exists"].clone(), report["data"]["prompts"].clone()), (json!(false), json!(0)));
    assert_eq!(report["index"], Value::Null);
    let daemon = &report["daemon"];
    assert_eq!((daemon["port"].clone(), daemon["listening"].clone(), daemon["healthz"].clone()), (json!(port), json!(false), json!(false)));
    assert_eq!((daemon["lock"]["held"].clone(), daemon["token_present"].clone()), (json!(false), json!(false)));
    assert_eq!(report["dashboard"]["source"], "embedded");
    assert_eq!(report["spool"], json!([]));
    assert_eq!(report["preferences"], json!([]));
    let claude = report["connectors"].as_array().expect("list").iter().find(|c| c["id"] == "claude-code").expect("claude-code").clone();
    assert_eq!((claude["connected"].clone(), claude["install"].clone(), claude["events"].clone()), (json!(false), Value::Null, json!(0)));
}

#[test]
fn a_held_lock_whose_pid_is_dead_is_a_problem() {
    let home = home();
    std::fs::create_dir_all(&home.settings.home).expect("mkdir");
    let mut child = std::process::Command::new("true").spawn().expect("spawn");
    let dead = child.id();
    child.wait().expect("wait");
    let mut lock = File::create(home.settings.lock_file()).expect("lock file");
    lock.try_lock().expect("take the lock");
    write!(lock, "{dead}").expect("write pid");

    let port = free_port();
    home.cmd_on(port, &["doctor"])
        .assert()
        .code(1)
        .stdout(contains(format!("pid {dead} written in it is not running")))
        .stdout(contains("lsof"));
    let (report, code) = report(&home, port);
    assert_eq!(code, 1);
    let lock_check = &report["daemon"]["lock"];
    assert_eq!(
        (lock_check["held"].clone(), lock_check["pid"].clone(), lock_check["pid_alive"].clone()),
        (json!(true), json!(dead), json!(false))
    );
    drop(lock);
}

#[test]
fn a_spec_that_does_not_load_is_a_problem_with_its_fallback() {
    let home = home();
    let sources = home.settings.user_sources();
    std::fs::create_dir_all(&sources).expect("mkdir");
    std::fs::write(sources.join("claude-code.yaml"), "id: [not valid").expect("write");

    let (report, code) = report(&home, free_port());
    assert_eq!(code, 1);
    let found = problems(&report);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with(&format!("Spec {}", sources.join("claude-code.yaml").display())), "{found:?}");
    assert!(report["problems"][0]["fix"].as_str().expect("fix").contains("bundled claude-code spec"), "{report}");
    home.cmd_on(free_port(), &["doctor"]).assert().code(1).stdout(contains("Problems (1)"));
}

#[test]
fn a_spool_backlog_is_a_problem_until_delivered() {
    let home = home();
    let spool = Spool::of(&home.settings);
    for text in ["one", "two"] {
        spool.append("claude-code", &json!({"hook_event_name": "UserPromptSubmit", "prompt": text})).expect("spool");
    }
    spool.append("codex", &json!({"prompt": "three"})).expect("spool");

    let (report, code) = report(&home, free_port());
    assert_eq!(code, 1);
    assert_eq!(report["spool"], json!([{"source": "claude-code", "payloads": 2}, {"source": "codex", "payloads": 1}]));
    assert_eq!(problems(&report), ["3 spooled hook payloads wait for delivery (claude-code 2, codex 1)."]);
    home.cmd_on(free_port(), &["doctor"]).assert().code(1).stdout(contains("backlog        claude-code 2, codex 1"));
}

#[test]
fn preferences_that_are_set_are_listed_with_where_they_come_from() {
    let home = home();
    let output = home.cmd_on(free_port(), &["doctor", "--json"]).env("GITPROMPT_WEEK_START", "sunday").output().expect("run");
    let report: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(report["preferences"], json!([{"key": "week_start", "value": "sunday", "source": "env", "env": "GITPROMPT_WEEK_START"}]));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_daemon_is_found_through_its_status() {
    let home = home();
    let daemon = home.daemon().await;
    let port = daemon.local_addr().port();
    let (report, code) = report(&home, port); // blocks this worker; the daemon runs on the others
    daemon.shutdown().await.expect("shutdown");
    assert_eq!(code, 0, "{report:#}");
    let d = &report["daemon"];
    assert_eq!((d["listening"].clone(), d["healthz"].clone(), d["token_present"].clone()), (json!(true), json!(true), json!(true)));
    assert_eq!(d["running"]["backend"], "rust");
    assert_eq!(d["running"]["home"], home.settings.home.display().to_string());
    assert_eq!((d["lock"]["held"].clone(), d["lock"]["pid"].clone()), (json!(true), json!(std::process::id())));
    assert_eq!(d["lock"]["pid_alive"], true);
    assert_eq!((report["data"]["db_exists"].clone(), report["index"]["stale"].clone()), (json!(true), json!(false)));
}

#[tokio::test(flavor = "multi_thread")]
async fn hooks_running_a_missing_forwarder_are_a_problem() {
    let home = home();
    let gone = home.settings.user_home.join("Old GitPrompt.app/gitprompt");
    let options = gitprompt_server::AppOptions { forwarder: Some(gone.clone()), ..home.options() };
    let app = App::new(home.settings.clone(), options).expect("app");
    app.service().connect("claude-code", false).await.expect("connect");
    drop(app);

    let (report, code) = report(&home, free_port());
    assert_eq!(code, 1);
    let claude = report["connectors"].as_array().expect("list").iter().find(|c| c["id"] == "claude-code").expect("claude-code").clone();
    assert_eq!(claude["connected"], true);
    assert_eq!(claude["install"], json!({"form": "forwarder", "path": gone.display().to_string()}));
    let found = problems(&report);
    assert!(found.iter().any(|p| p.contains("connected hooks can't deliver")), "{found:?}");
    assert!(found.iter().any(|p| p.contains(&format!("the hooks run {}, which does not exist", gone.display()))), "{found:?}");
}
