//! `gitprompt why <commit>` over a real repository (commits at set dates) and prompts sent in it at
//! set times, run in-process in a chosen folder: a commit made during a prompt's turn is traced to
//! it, one made after every turn is likely, one before every prompt is made by hand; a revision
//! resolves with git here, a hash anywhere.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use std::path::Path;
use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::inprocess::{Call, Ran, run_cli, seed_with};
use common::repo::{Repo, repo};
use common::{Home, home};
use gitprompt_insights::stories::project_id;
use gitprompt_server::{AppOptions, FixedClock};

/// When the first prompt is sent.
const T0: &str = "2026-11-17T12:00:00Z";

fn at(minutes: i64, seconds: i64) -> Timestamp {
    T0.parse::<Timestamp>().unwrap() + SignedDuration::from_mins(minutes) + SignedDuration::from_secs(seconds)
}

fn iso(minutes: i64, seconds: i64) -> String {
    at(minutes, seconds).to_string()
}

/// A prompt sent in `session` at `sent`, its turn ended a minute later; real git facts of `cwd`.
fn prompt(home: &Home, cwd: &str, session: &str, text: &str, sent: Timestamp) {
    let event = |name: &str, extra: Value| {
        let mut payload = json!({"hook_event_name": name, "session_id": session, "prompt_id": format!("{session}-{sent}"), "cwd": cwd});
        payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        ("claude-code", payload)
    };
    let real_git = |at: Timestamp| AppOptions { clock: Arc::new(FixedClock(at)), ..AppOptions::default() };
    seed_with(home, real_git(sent), vec![event("UserPromptSubmit", json!({"prompt": text}))], &[]);
    seed_with(home, real_git(sent + SignedDuration::from_mins(1)), vec![event("Stop", json!({}))], &[]);
}

fn name(home: &Home, session: &str, name: &str) {
    seed_with(home, home.options(), vec![], &[(session, name)]);
}

/// The repository's commits by what they stand for.
struct Commits {
    by_hand: String,
    traced: String,
    likely: String,
    two_at_work: String,
}

/// One repository: a commit an hour before any prompt, one during the turn of "add a retry loop"
/// (session "Rust switch"), one an hour after it, and one while two sessions were at work.
fn fixture() -> (Home, Repo, Commits) {
    let (home, repo) = (home(), repo("app"));
    let by_hand = repo.commit("Tidy the readme", &iso(-60, 0));
    prompt(&home, repo.path(), "s1", "add a retry loop to the uploader\nwith backoff\nand a test\nthen docs", at(0, 0));
    let traced = repo.commit("Add the retry loop", &iso(0, 30));
    let likely = repo.commit("Fix a typo", &iso(60, 0));
    prompt(&home, repo.path(), "s2", "write the docs", at(120, 0));
    prompt(&home, repo.path(), "s3", "add the metrics", at(120, 10));
    let two_at_work = repo.commit("Docs and metrics", &iso(120, 30));
    name(&home, "s1", "Rust switch");
    (home, repo, Commits { by_hand, traced, likely, two_at_work })
}

fn why_in(home: &Home, cwd: &Path, args: &[&str]) -> Ran {
    let args: Vec<&str> = std::iter::once("why").chain(args.iter().copied()).collect();
    run_cli(home, &args, Call { cwd: cwd.to_path_buf(), ..Call::at(at(180, 0)) })
}

fn short(hash: &str) -> &str {
    &hash[..7]
}

#[test]
fn a_traced_commit_names_its_prompt_and_session_on_the_first_line() {
    let (home, repo, commits) = fixture();
    let ran = why_in(&home, &repo.root, &["HEAD~2"]);
    assert_eq!((ran.code, ran.err.as_str()), (0, ""));
    let hash = short(&commits.traced);
    let link = format!("http://127.0.0.1:7878/projects/{}?tab=commits#commit-{hash}", project_id(repo.path()));
    assert_eq!(
        ran.out,
        format!(
            "\
{hash} ← \"add a retry loop to the uploader\" · session Rust switch · 17 Nov
commit   Add the retry loop  (app)
prompt   add a retry loop to the uploader
         with backoff
         and a test
         …
open     {link}
"
        )
    );
}

#[test]
fn a_commit_after_every_turn_is_likely_and_says_so_in_words() {
    let (home, repo, commits) = fixture();
    let ran = why_in(&home, &repo.root, &[short(&commits.likely)]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(
        lines[0],
        format!("{} ← likely \"add a retry loop to the uploader\" · session Rust switch · 17 Nov", short(&commits.likely))
    );
    assert!(
        lines.contains(&"likely   not certain: committed after the prompt's work ended, or rewritten since (amend, rebase)"),
        "{}",
        ran.out
    );
}

#[test]
fn a_likely_commit_names_the_other_prompt_that_could_have_made_it() {
    let (home, repo, commits) = fixture();
    let ran = why_in(&home, &repo.root, &["HEAD"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(lines[0], format!("{} ← likely \"add the metrics\" · session add the metrics · 17 Nov", short(&commits.two_at_work)));
    assert!(
        lines.contains(
            &"likely   not certain: another prompt was at work then and could have made it: \"write the docs\" · session write the docs"
        ),
        "{}",
        ran.out
    );
}

#[test]
fn a_commit_before_every_prompt_was_made_by_hand() {
    let (home, repo, commits) = fixture();
    let ran = why_in(&home, &repo.root, &[&commits.by_hand]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(
        lines[..3],
        [
            format!("{} ← made by hand · 17 Nov", short(&commits.by_hand)).as_str(),
            "commit   Tidy the readme  (app)",
            "by hand  no prompt's work was running when it was made",
        ]
    );
}

#[test]
fn a_hash_works_outside_any_repository() {
    let (home, _repo, commits) = fixture();
    let ran = why_in(&home, Path::new("/"), &[short(&commits.traced)]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(ran.out.starts_with(&format!("{} ← \"add a retry loop", short(&commits.traced))), "{}", ran.out);
}

#[test]
fn a_revision_outside_a_repository_says_so() {
    let (home, _repo, _) = fixture();
    let outside = tempfile::tempdir().unwrap();
    let ran = why_in(&home, outside.path(), &["HEAD"]);
    assert_eq!((ran.code, ran.out.as_str()), (1, ""));
    assert!(ran.err.contains("is not in a git repository, so \"HEAD\" names no commit here"), "{}", ran.err);
}

#[test]
fn a_revision_the_repository_lacks_and_a_commit_no_project_has_say_so() {
    let (home, repo, _) = fixture();
    let ran = why_in(&home, &repo.root, &["HEAD~9"]);
    assert_eq!((ran.code, ran.err.as_str()), (1, "This repository has no commit \"HEAD~9\".\n"));
    let ran = why_in(&home, Path::new("/"), &["ffff0000"]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.starts_with("No commit \"ffff0000\" among your projects' commits"), "{}", ran.err);
}

#[test]
fn a_revision_no_project_has_names_the_commit_it_stood_for() {
    let (home, repo, _) = fixture();
    let late = repo.commit("Long after", &iso(24 * 60, 0));
    let ran = why_in(&home, &repo.root, &["HEAD"]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.starts_with(&format!("No commit {} (HEAD) among your projects' commits", short(&late))), "{}", ran.err);
}

#[test]
fn a_hash_in_two_projects_lists_the_candidates() {
    let (home, repo, commits) = fixture();
    let clone = repo.clone_as("fork");
    prompt(&home, clone.path(), "s9", "look around the fork", at(0, 10));
    let hash = short(&commits.traced);
    let ran = why_in(&home, Path::new("/"), &[hash]);
    assert_eq!((ran.code, ran.out.as_str()), (1, ""));
    let mut lines = ran.err.lines();
    assert_eq!(lines.next(), Some(format!("2 commits start with \"{hash}\"; run it again with more of the hash:").as_str()));
    let mut projects: Vec<&str> = lines.map(|line| line.split_whitespace().nth(1).unwrap()).collect();
    projects.sort_unstable();
    assert_eq!(projects, ["app", "fork"]);
}

#[test]
fn json_prints_the_lookup_for_scripts() {
    let (home, repo, commits) = fixture();
    let ran = why_in(&home, &repo.root, &["HEAD~2", "--json"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let found: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!((&found["found"], &found["commit"]["how"]), (&json!("one"), &json!("traced")));
    assert_eq!((&found["commit"]["commit"]["hash"], &found["commit"]["session"]), (&json!(short(&commits.traced)), &json!("s1")));
    assert_eq!(found["commit"]["project"]["name"], "app");
}
