//! `gitprompt why --branch`, run in-process over a home seeded through the server's Service: the
//! prompts you typed on a branch, printed as the Markdown block the dashboard copies.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use common::inprocess::{Call, Ran, run_cli};
use common::{Home, home};
use gitprompt_cli::Cli;
use gitprompt_core::JsonMap;
use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_server::{App, AppOptions, FixedClock, GitProvider, NoGit};

const AWS_KEY: &str = concat!("AKIA", "QWERTYUIOPASDFGH");

fn t0() -> Timestamp {
    "2026-11-17T12:00:00Z".parse().unwrap()
}

/// Every folder a repository of its own, checked out on one branch.
struct OnBranch(&'static str);

impl GitProvider for OnBranch {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        let Some(cwd) = cwd else { return JsonMap::new() };
        JsonMap::from_iter([("git_root".into(), cwd.to_string_lossy().into()), ("git_branch".into(), self.0.into())])
    }
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
        NoGit.commits(root, start, end, since)
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        Vec::new()
    }
}

/// A prompt typed in `cwd` on `branch`, `minutes` after [`t0`].
struct Typed {
    branch: &'static str,
    cwd: &'static str,
    text: String,
    minutes: i64,
}

fn seed(home: &Home, prompts: Vec<Typed>) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for (n, typed) in prompts.into_iter().enumerate() {
        let at = t0() + SignedDuration::from_mins(typed.minutes);
        let options = AppOptions { clock: Arc::new(FixedClock(at)), git: Arc::new(OnBranch(typed.branch)), ..home.options() };
        let service = App::new(home.settings.clone(), options).expect("app").service();
        let payload: Value = json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt_id": format!("p{n}"), "cwd": typed.cwd, "prompt": typed.text});
        runtime.block_on(service.ingest("claude-code", payload)).expect("ingest");
    }
}

fn typed(branch: &'static str, cwd: &'static str, text: &str, minutes: i64) -> Typed {
    Typed { branch, cwd, text: text.to_owned(), minutes }
}

fn why(home: &Home, args: &[&str]) -> Ran {
    let args: Vec<&str> = std::iter::once("why").chain(args.iter().copied()).collect();
    run_cli(home, &args, Call::at(t0() + SignedDuration::from_mins(90)))
}

#[test]
fn prints_the_prompts_behind_a_branch_with_secrets_masked_and_paths_relative() {
    let home = home();
    seed(
        &home,
        vec![
            typed("timing-ledger", "/work/api", &format!("deploy with {AWS_KEY}, then edit /work/api/src/lib.rs"), 0),
            typed("main", "/work/api", "an older idea on main", 1),
            typed("timing-ledger", "/work/api", "block the flaky kinds", 2),
        ],
    );
    let ran = why(&home, &["--branch", "timing-ledger"]);
    assert_eq!((ran.code, ran.err.as_str()), (0, ""));
    assert!(ran.out.starts_with("### Prompts behind timing-ledger\n\n2 prompts · 0 commits\n"), "{}", ran.out);
    assert!(ran.out.contains("deploy with [hidden AWS access key], then edit src/lib.rs"), "{}", ran.out);
    assert!(ran.out.contains("<summary>2. block the flaky kinds</summary>") && !ran.out.contains("older idea"), "{}", ran.out);
    assert!(!ran.out.contains(AWS_KEY) && !ran.out.contains("/work/api"), "{}", ran.out);
}

#[test]
fn the_branch_is_read_in_the_project_where_you_last_typed_on_it_unless_one_is_named() {
    let home = home();
    seed(&home, vec![typed("main", "/work/api", "in the api", 0), typed("main", "/work/app", "in the app", 1)]);
    let latest = why(&home, &["--branch", "main"]);
    assert!(latest.out.contains("in the app") && !latest.out.contains("in the api"), "{}", latest.out);
    let named = why(&home, &["--branch", "main", "--project", "api"]);
    assert!(named.out.contains("in the api") && !named.out.contains("in the app"), "{}", named.out);
}

#[test]
fn a_branch_without_prompts_exits_1_with_a_note() {
    let home = home();
    seed(&home, vec![typed("main", "/work/api", "in the api", 0)]);
    let ran = why(&home, &["--branch", "nowhere"]);
    assert_eq!((ran.code, ran.out.as_str()), (1, ""));
    assert_eq!(ran.err, "No prompts you typed were recorded on branch \"nowhere\".\n");
    let elsewhere = why(&home, &["--branch", "main", "--project", "app"]);
    assert_eq!(elsewhere.err, "No prompts you typed were recorded on branch \"main\" in project \"app\".\n");
}

#[test]
fn why_wants_a_commit_or_a_branch_never_both() {
    let parses = |args: &[&str]| Cli::try_parse_from(["gitprompt", "why"].iter().chain(args)).is_ok();
    assert!(!parses(&[]));
    assert!(!parses(&["HEAD", "--branch", "main"]));
    assert!(!parses(&["--branch", "main", "--json"]), "--json is the commit's");
    assert!(!parses(&["HEAD", "--project", "api"]), "--project is the branch's");
    assert!(parses(&["HEAD", "--json"]) && parses(&["--branch", "main", "--project", "api"]));
}
