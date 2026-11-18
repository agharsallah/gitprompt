//! `GET /api/commits/{hash}` and the commits of `/api/find`, over a repository whose history and
//! remote are given (no git run): a full hash, a prefix, an ambiguous prefix, an unknown one.
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

use gitprompt_core::JsonMap;
use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_server::{GitProvider, NoGit};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::{Api, harness, project_id};

const REPO: &str = "/work/app";
/// When the one prompt is sent; its turn ends a minute later.
const SENT: &str = "2026-03-02T09:00:00Z";

/// `(hash, subject, time)`: two made during the turn, one half an hour later; two share `abcd`.
const COMMITS: &[(&str, &str, &str)] = &[
    ("abcd123", "Add the retry loop", "2026-03-02T09:00:20+00:00"),
    ("d4d4d4d", "Wire the uploader", "2026-03-02T09:00:40+00:00"),
    ("abcd456", "Fix a typo", "2026-03-02T09:30:00+00:00"),
];

/// Every folder is a git repository on `main` with a GitHub `origin` and [`COMMITS`] as history.
struct Forge;

impl GitProvider for Forge {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        let Some(cwd) = cwd else { return JsonMap::new() };
        let mut context = JsonMap::new();
        context.insert("git_root".into(), cwd.to_string_lossy().into());
        context.insert("git_branch".into(), "main".into());
        context.insert("git_remote".into(), "git@github.com:acme/app.git".into());
        context
    }
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
        NoGit.commits(root, start, end, since)
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        let commit = |&(hash, subject, time): &(&str, &str, &str)| Commit {
            hash: hash.into(),
            subject: subject.into(),
            author: "t".into(),
            time: time.into(),
            ..Commit::default()
        };
        COMMITS.iter().map(commit).collect()
    }
}

/// One finished prompt in [`REPO`].
async fn app() -> Api {
    let api = harness().git(Forge).clock(SENT.parse().unwrap()).start();
    let event = |name: &str, extra: Value| {
        common::with(&json!({"hook_event_name": name, "prompt_id": "p1", "session_id": "s1", "cwd": REPO}), extra)
    };
    api.ingest("claude-code", event("UserPromptSubmit", json!({"prompt": "add a retry loop to the uploader"}))).await;
    api.ingest("claude-code", event("Stop", json!({}))).await;
    api
}

#[tokio::test]
async fn a_full_hash_finds_its_trace_its_project_and_its_page_on_the_forge() {
    let api = app().await;
    let found = api.json("/api/commits/abcd123").await;
    assert_eq!(found["found"], "one");
    let commit = &found["commit"];
    assert_eq!((&commit["commit"]["subject"], &commit["how"]), (&json!("Add the retry loop"), &json!("traced")));
    assert_eq!(commit["prompt"]["text"], "add a retry loop to the uploader");
    assert_eq!((&commit["session"], &commit["project"]["id"]), (&json!("s1"), &json!(project_id(REPO))));
    assert_eq!(commit["url"], "https://github.com/acme/app/commit/abcd123");
}

#[tokio::test]
async fn a_prefix_finds_the_one_commit_it_names() {
    let api = app().await;
    let found = api.json("/api/commits/D4D4").await;
    assert_eq!((&found["found"], &found["commit"]["commit"]["hash"]), (&json!("one"), &json!("d4d4d4d")));
}

#[tokio::test]
async fn an_ambiguous_prefix_gives_the_candidates_newest_first() {
    let api = app().await;
    let found = api.json("/api/commits/abcd").await;
    let hashes: Vec<&Value> = found["candidates"].as_array().expect("candidates").iter().map(|c| &c["commit"]["hash"]).collect();
    assert_eq!((&found["found"], hashes), (&json!("many"), vec![&json!("abcd456"), &json!("abcd123")]));
}

#[tokio::test]
async fn an_unknown_hash_is_404_and_what_is_not_a_hash_400() {
    let api = app().await;
    let mut statuses = Vec::new();
    for hash in ["ffff", "abc", "not-a-hash", "HEAD"] {
        statuses.push(api.get(&format!("/api/commits/{hash}")).send().await.status);
    }
    assert_eq!(statuses, [404, 400, 400, 400]);
}

#[tokio::test]
async fn find_lists_a_commit_by_the_first_words_of_its_subject_and_by_seven_characters_of_its_hash() {
    let api = app().await;
    let hashes = |found: &Value| -> Vec<String> {
        found["commits"].as_array().expect("commits").iter().map(|h| h["target"]["hash"].as_str().unwrap().to_owned()).collect()
    };
    let by_subject = api.json("/api/find?q=add%20the&kinds=commits").await;
    assert_eq!(hashes(&by_subject), ["abcd123"]);
    assert_eq!(by_subject["commits"][0]["target"], json!({"kind": "commit", "project": project_id(REPO), "hash": "abcd123"}));
    assert_eq!(hashes(&api.json("/api/find?q=d4d4d4d&kinds=commits").await), ["d4d4d4d"]);
    assert_eq!(api.json("/api/find?q=add%20the&kinds=prompts").await["commits"], json!([]), "not asked for");
}
