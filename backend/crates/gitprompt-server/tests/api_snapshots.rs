//! Snapshots of every read endpoint the dashboard calls, over the connector conformance fixtures.
//!
//! Every fixture of every tool (`backend/testdata/fixtures/<tool>/`) is ingested through the real
//! receiver, then each endpoint's JSON is compared with its snapshot in `tests/snapshots/`. Where
//! `ts_types.rs` keeps the dashboard's *types* in step with the Rust wire types, these keep the
//! *values*: a renamed, dropped or re-derived field shows up as a snapshot diff in review.
//!
//! Time is not redacted but controlled: the app runs on a [`ManualClock`] that starts at [`START`]
//! and moves [`STEP`] before each fixture, so arrival times, the durations between them and the
//! hourly heartbeat are exact values here, the same on every machine and every day. Only the temp
//! directory is replaced (by `[root]`). Never read the system clock on a path these snapshots
//! cover: take "now" from the app's `Clock` (`gitprompt_server::clock`).
//!
//! Update after an intended change: `just snapshots`, then review the diff of `tests/snapshots/`.
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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gitprompt_core::JsonMap;
use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_server::{GitProvider, ManualClock, NoGit};
use jiff::{SignedDuration, Timestamp};
use serde_json::Value;

use common::{Api, Harness, harness, project_id};

/// The tools whose fixtures are ingested, in this order (ids follow it).
const TOOLS: &[&str] = &["claude-code", "codex", "copilot-cli", "cursor"];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/fixtures")
}

/// The payloads of one tool's fixtures, in file-name order.
fn payloads(tool: &str) -> Vec<Value> {
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(fixtures_dir().join(tool)).expect("fixtures dir").map(|e| e.expect("entry").path()).collect();
    files.sort();
    files
        .iter()
        .filter(|f| f.extension().is_some_and(|e| e == "json"))
        .map(|f| {
            let fixture: Value = serde_json::from_str(&std::fs::read_to_string(f).expect("read fixture")).expect("fixture JSON");
            fixture["payload"].clone()
        })
        .collect()
}

/// When the first fixture arrives, and the time between two arrivals.
const START: &str = "2026-01-05T09:00:00Z";
const STEP: SignedDuration = SignedDuration::from_secs(7);

/// An app holding every fixture of every tool, each arriving [`STEP`] after the previous one on a
/// clock the test moves, so every `received_at` and duration between arrivals is the same each run.
async fn loaded() -> Api {
    loaded_with(harness()).await
}

/// [`loaded`], on an app built from `harness`.
async fn loaded_with(harness: Harness) -> Api {
    loaded_on(harness, &ManualClock::new(START.parse().expect("instant"))).await
}

/// [`loaded_with`], on `clock`, which the test can move on afterwards.
async fn loaded_on(harness: Harness, clock: &ManualClock) -> Api {
    let api = harness.moving_clock(clock).start();
    for tool in TOOLS {
        for payload in payloads(tool) {
            clock.advance(STEP);
            api.ingest(tool, payload).await;
        }
    }
    api
}

// --- normalizing what depends on the run ---

/// The temp directory is the only value that differs between runs.
fn normalize(value: &mut Value, root: &str) {
    match value {
        Value::String(text) if text.contains(root) => *text = text.replace(root, "[root]"),
        Value::Array(items) => items.iter_mut().for_each(|v| normalize(v, root)),
        Value::Object(map) => map.values_mut().for_each(|v| normalize(v, root)),
        _ => {}
    }
}

/// GET `path` (must answer 200), normalized.
async fn get(api: &Api, path: &str) -> Value {
    let mut body = api.json(path).await;
    normalize(&mut body, &api.root().to_string_lossy());
    body
}

/// The first prompt `source` sent (the list is newest first).
fn first_of<'a>(prompts: &'a Value, source: &str) -> &'a Value {
    prompts.as_array().expect("list").iter().rfind(|p| p["source"] == source).unwrap_or_else(|| panic!("no {source} prompt"))
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter().fold(value, |v, k| &v[*k]).as_str().unwrap_or_else(|| panic!("no text at {path:?}"))
}

// --- the snapshots ---

#[tokio::test]
async fn prompts_and_their_details() {
    let api = loaded().await;
    let prompts = get(&api, "/api/prompts?limit=200").await;
    insta::assert_json_snapshot!("prompts", prompts);
    for tool in TOOLS {
        let id = first_of(&prompts, tool)["id"].as_i64().expect("id");
        insta::assert_json_snapshot!(format!("prompt_detail_{tool}"), get(&api, &format!("/api/prompts/{id}")).await);
    }
}

#[tokio::test]
async fn stats_and_telemetry() {
    let api = loaded().await;
    insta::assert_json_snapshot!("stats", get(&api, "/api/stats").await);
    for group in ["source", "origin", "outcome", "model", "tool_kind", "session"] {
        insta::assert_json_snapshot!(format!("stats_by_{group}"), get(&api, &format!("/api/stats?group_by={group}")).await);
    }
    insta::assert_json_snapshot!("telemetry", get(&api, "/api/stats/telemetry").await);
}

/// This week against last week: the first two tools' fixtures arrive a week before the other two's,
/// and the stats of the last seven days carry the seven before them.
#[tokio::test]
async fn stats_with_the_week_before() {
    let clock = ManualClock::new(START.parse().expect("instant"));
    let api = harness().moving_clock(&clock).start();
    for (n, tool) in TOOLS.iter().enumerate() {
        if n == 2 {
            clock.advance(SignedDuration::from_hours(7 * 24));
        }
        for payload in payloads(tool) {
            clock.advance(STEP);
            api.ingest(tool, payload).await;
        }
    }
    let since = clock_now(&clock) - SignedDuration::from_hours(7 * 24);
    insta::assert_json_snapshot!("stats_previous", get(&api, &format!("/api/stats?previous=true&since={since}")).await);
}

/// The clock's instant (a `ManualClock` is a `Clock`).
fn clock_now(clock: &ManualClock) -> Timestamp {
    gitprompt_server::Clock::now(clock)
}

#[tokio::test]
async fn stories_lineage_and_search() {
    let api = loaded().await;
    let prompts = api.json("/api/prompts?limit=200").await;
    for tool in TOOLS {
        let prompt = first_of(&prompts, tool);
        let session = text(prompt, &["fields", "session_id"]);
        insta::assert_json_snapshot!(format!("session_story_{tool}"), get(&api, &format!("/api/stories/session/{session}")).await);
        let id = prompt["id"].as_i64().expect("id");
        insta::assert_json_snapshot!(format!("lineage_{tool}"), get(&api, &format!("/api/stories/prompt/{id}")).await);
    }
    let project = project_id(Path::new("/work/app").to_str().expect("utf-8"));
    insta::assert_json_snapshot!("project_story", get(&api, &format!("/api/stories/project/{project}")).await);
    insta::assert_json_snapshot!("search", get(&api, "/api/search?q=retry").await);
}

/// Every folder a git repository on `main` with a GitHub `origin`; `/work/app`'s history is
/// [`FIXTURE_COMMITS`] (within the window asked for, as `git log --since --until` would answer),
/// the others have none, so a hash names one project's commit.
struct FixtureGit;

/// `(hash, subject, time, paths)` around the `/work/app` prompts (09:00:07 to 09:03:51, the turns
/// of prompts 1 and 9 run 09:00:07–09:00:21 and 09:02:20–09:02:48): one by hand before the first
/// prompt, two made during a turn, one half an hour after the last prompt.
const FIXTURE_COMMITS: &[(&str, &str, &str, &[&str])] = &[
    ("a1a1a1a", "Scaffold the app", "2026-01-05T08:15:00+00:00", &["Cargo.toml", "README.md"]),
    ("b2b2b2b", "Add the retry loop", "2026-01-05T09:00:15+00:00", &["src/upload.rs"]),
    ("c3c3c3c", "Wire the uploader", "2026-01-05T09:02:40+00:00", &["src/main.rs", "src/upload.rs"]),
    ("d4d4d4d", "Fix a typo in the README", "2026-01-05T09:30:00+00:00", &["README.md"]),
];

impl GitProvider for FixtureGit {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        let Some(cwd) = cwd else { return JsonMap::new() };
        let mut context = JsonMap::new();
        context.insert("git_root".into(), cwd.to_string_lossy().into());
        context.insert("git_branch".into(), "main".into());
        context.insert("git_remote".into(), "https://github.com/acme/app.git".into());
        context
    }
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
        NoGit.commits(root, start, end, since)
    }
    fn history(&self, root: &Path, since: &str, until: &str) -> Vec<Commit> {
        if root != Path::new("/work/app") {
            return Vec::new();
        }
        let instant = |text: &str| text.parse::<Timestamp>().expect("instant");
        let window = instant(since)..=instant(until);
        FIXTURE_COMMITS
            .iter()
            .filter(|(_, _, time, _)| window.contains(&instant(time)))
            .map(|&(hash, subject, time, paths)| Commit {
                hash: hash.into(),
                subject: subject.into(),
                author: "t".into(),
                time: time.into(),
                authored: time.into(),
                files: paths.len() as u64,
                insertions: 3,
                deletions: 1,
                paths: paths.iter().map(|p| (*p).to_owned()).collect(),
            })
            .collect()
    }
}

/// The hashes of a commit list, in its groups' order.
fn listed(commits: &Value) -> Vec<&str> {
    let groups = commits["groups"].as_array().expect("groups");
    groups.iter().flat_map(|g| g["commits"].as_array().expect("commits")).map(|t| text(t, &["commit", "hash"])).collect()
}

#[tokio::test]
async fn commits_with_the_prompt_behind_each() {
    let clock = ManualClock::new(START.parse().expect("instant"));
    let api = loaded_on(harness().git(FixtureGit), &clock).await;
    // The list reads up to now: move it past the last fixture commit (09:30).
    clock.advance(SignedDuration::from_hours(1));
    let path = format!("/api/stories/project/{}/commits", project_id("/work/app"));
    let commits = get(&api, &path).await;
    // Every commit of the project exactly once, newest first.
    assert_eq!(listed(&commits), ["d4d4d4d", "c3c3c3c", "b2b2b2b", "a1a1a1a"]);
    // The project story gives each prompt the same commits the list traces back to it.
    let story = get(&api, &format!("/api/stories/project/{}", project_id("/work/app"))).await;
    assert_eq!(owners_in_story(&story), owners_in_list(&commits));
    insta::assert_json_snapshot!("project_commits", commits);
    insta::assert_json_snapshot!("project_commits_by_day_readme", get(&api, &format!("{path}?group=day&q=readme")).await);
}

#[tokio::test]
async fn the_prompts_behind_a_branch() {
    let api = loaded_with(harness().git(FixtureGit)).await;
    let project = project_id("/work/app");
    let response = api.get("/api/export/branch").query(&[("project", project.as_str()), ("branch", "main")]).send().await;
    assert_eq!(response.status, 200, "{}", response.text());
    insta::assert_snapshot!("export_branch", response.text().replace(&*api.root().to_string_lossy(), "[root]"));
}

/// hash -> prompt id, from the prompts' side (a project story).
fn owners_in_story(story: &Value) -> BTreeMap<String, i64> {
    let entries = story["phases"].as_array().expect("phases").iter().flat_map(|p| p["items"].as_array().expect("items"));
    entries
        .flat_map(|e| {
            e["commits"].as_array().expect("commits").iter().map(move |c| (text(c, &["hash"]).to_owned(), e["id"].as_i64().expect("id")))
        })
        .collect()
}

/// hash -> prompt id, from the commits' side (a commit list), commits made by hand left out.
fn owners_in_list(commits: &Value) -> BTreeMap<String, i64> {
    let groups = commits["groups"].as_array().expect("groups");
    let traces = groups.iter().flat_map(|g| g["commits"].as_array().expect("commits"));
    traces.filter_map(|t| Some((text(t, &["commit", "hash"]).to_owned(), t["prompt"]["id"].as_i64()?))).collect()
}

#[tokio::test]
async fn find_prompts_sessions_and_projects() {
    let api = loaded().await;
    insta::assert_json_snapshot!("find", get(&api, "/api/find?q=retry").await);
    insta::assert_json_snapshot!("find_recent", get(&api, "/api/find").await);
}

#[tokio::test]
async fn a_commit_by_its_hash_and_in_find() {
    let api = loaded_with(harness().git(FixtureGit)).await;
    insta::assert_json_snapshot!("commit", get(&api, "/api/commits/c3c3c3c").await);
    insta::assert_json_snapshot!("find_commits", get(&api, "/api/find?q=retry").await);
}

#[tokio::test]
async fn connectors_fields_and_settings() {
    let api = loaded().await;
    insta::assert_json_snapshot!("sources", get(&api, "/api/sources").await);
    insta::assert_json_snapshot!("sources_recent", get(&api, "/api/sources/recent").await);
    insta::assert_json_snapshot!("sources_problems", get(&api, "/api/sources/problems").await);
    insta::assert_json_snapshot!("fields", get(&api, "/api/fields").await);
    insta::assert_json_snapshot!("settings", get(&api, "/api/settings").await);
}

/// What the page reported on one day and the next: both days, oldest first, the range around them.
#[tokio::test]
async fn usage_counts_over_two_days() {
    let clock = ManualClock::new(START.parse().expect("instant"));
    let api = harness().moving_clock(&clock).start();
    let report = |hits: Value| api.post("/api/usage").dashboard().json(serde_json::json!({ "hits": hits })).send();
    let first = report(serde_json::json!([{"action": "copy.prompt", "n": 2}, {"action": "panel.seen.rhythm", "n": 1}])).await;
    assert_eq!(first.status, 204);
    clock.advance(SignedDuration::from_hours(24));
    report(serde_json::json!([{"action": "copy.prompt", "n": 1}, {"action": "reader.tab.events", "n": 3}])).await;
    insta::assert_json_snapshot!("usage", get(&api, "/api/usage?days=7").await);
}
