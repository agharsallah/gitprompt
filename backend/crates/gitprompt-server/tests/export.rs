//! GET /api/export/{kind}: every kind in every format, with secrets, email addresses and home paths
//! planted in the synthetic data (ADR-0018). Port of test_export_routes.py; the Python
//! kind × format parametrization is one test per kind here, collecting every leak it finds.

mod common;

use std::path::PathBuf;

use common::{Api, Resp, git, harness, otlp, project_id};
use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const AWS_KEY: &str = concat!("AKIA", "QWERTYUIOPASDFGH");
const GITHUB_TOKEN: &str = concat!("ghp_", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8");
const EMAIL: &str = "jane.doe@example.com";
const ACCOUNT_EMAIL: &str = "account.owner@example.org";
const ORG_ID: &str = "org-5f1c0ffee";
const OTHER_HOME: &str = "/Users/jdoe";
const NEVER: [&str; 6] = [AWS_KEY, GITHUB_TOKEN, EMAIL, ACCOUNT_EMAIL, ORG_ID, OTHER_HOME];
const FORMATS: [&str; 5] = ["md", "html", "csv", "json", "svg"];
/// The app clock: names the downloaded files.
const NOW: &str = "2026-03-04T12:00:00Z";

fn git_as_jane(repo: &std::path::Path, args: &[&str], when: &str) {
    let mut all = vec!["-c", "user.name=Jane Doe", "-c", "user.email=jane.doe@example.com"];
    all.extend_from_slice(args);
    git(repo, &all, Some(when));
}

/// A project inside the home folder (like most), one session with a planted prompt, two tool calls
/// (one a subagent naming a home path), a task notification and an account-identifying telemetry
/// resource; a commit by Jane during the turn.
async fn data() -> (Api, PathBuf) {
    let now: Timestamp = NOW.parse().expect("time");
    let api = harness().clock(now).start();
    let repo = api.settings.user_home.join("code/app");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "-b", "main"], None);
    std::fs::write(repo.join("a.txt"), "a\n").expect("write");
    git(&repo, &["add", "."], None);
    git_as_jane(&repo, &["commit", "-m", "initial"], "2026-03-01T08:00:00+00:00");

    let home = api.settings.user_home.display().to_string();
    let cwd = repo.display().to_string();
    let planted = format!("deploy with {AWS_KEY} and {GITHUB_TOKEN}, mail {EMAIL}, notes in {OTHER_HOME}/notes.txt and {cwd}/src/main.py");
    let sends = [
        ("UserPromptSubmit", "p1", "2026-03-02T09:00:00", json!({"prompt": planted})),
        ("PostToolUse", "p1", "2026-03-02T09:00:10", json!({"tool_name": "Edit", "tool_input": {"file_path": format!("{cwd}/a.txt")}})),
        (
            "PostToolUse",
            "p1",
            "2026-03-02T09:00:20",
            json!({"tool_name": "Agent", "tool_input": {"description": "Look", "subagent_type": "Explore", "prompt": format!("check {home}/x")}}),
        ),
        ("Stop", "p1", "2026-03-02T09:01:00", json!({})),
        (
            "UserPromptSubmit",
            "p2",
            "2026-03-02T09:02:00",
            json!({"prompt": format!("<task-notification>done, ping {EMAIL}</task-notification>")}),
        ),
    ];
    let mut statements = String::new();
    let mut event_id = 0;
    for (event, pid, at, extra) in sends {
        let base = json!({"hook_event_name": event, "prompt_id": pid, "session_id": "s1", "cwd": cwd, "transcript_path": format!("{home}/.claude/projects/app/s1.jsonl")});
        api.ingest("claude-code", common::with(&base, extra)).await;
        let (table, row) = if event == "UserPromptSubmit" {
            ("prompt", format!("json_extract(fields, '$.prompt_id') = '{pid}'"))
        } else {
            event_id += 1;
            ("event", format!("id = {event_id}"))
        };
        statements += &format!("update {table} set received_at = '{at}.000000+00:00' where {row};");
    }
    api.sql(&statements);
    let resource = json!({"service.name": "claude-code", "user.email": ACCOUNT_EMAIL, "organization.id": ORG_ID, "user.id": "u-123456"});
    api.ingest_otlp(
        "claude-code-otel",
        otlp(resource, json!({"event.name": "api_request", "prompt.id": "p1", "session.id": "s1", "model": "claude-haiku"})),
    )
    .await;
    std::fs::write(repo.join("b.txt"), "b\n").expect("write");
    git(&repo, &["add", "."], None);
    git_as_jane(&repo, &["commit", "-m", "Add b"], "2026-03-02T09:00:40+00:00");
    (api, repo)
}

/// The query of an export of `kind`: its id, the user's origins for selections, plus `extra`.
fn params(kind: &str, repo: &std::path::Path, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = match kind {
        "session" => vec![("id".into(), "s1".into())],
        "project" => vec![("id".into(), project_id(repo.to_str().expect("utf-8")))],
        _ => vec![("origin".into(), "typed".into()), ("origin".into(), "slash_command".into())],
    };
    out.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    out
}

async fn fetch(api: &Api, kind: &str, repo: &std::path::Path, extra: &[(&str, &str)]) -> Resp {
    let pairs = params(kind, repo, extra);
    let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let response = api.get(&format!("/api/export/{kind}")).query(&borrowed).send().await;
    assert_eq!(response.status, 200, "{kind} {extra:?}: {}", response.text());
    response
}

/// (format, value) of every forbidden value found in the default export of `kind`.
async fn default_leaks(kind: &str) -> Vec<(String, String)> {
    let (api, repo) = data().await;
    let home = api.settings.user_home.display().to_string();
    let mut leaks = Vec::new();
    for format in FORMATS {
        let body = fetch(&api, kind, &repo, &[("format", format)]).await.text();
        let forbidden =
            NEVER.iter().map(|s| s.to_string()).chain([home.clone(), "deploy with".into(), "Jane Doe".into(), "u-123456".into()]);
        leaks.extend(forbidden.filter(|v| body.contains(v.as_str())).map(|v| (format.to_string(), v)));
    }
    leaks
}

#[tokio::test]
async fn a_default_session_export_leaves_out_secrets_emails_paths_and_text_in_every_format() {
    assert_eq!(default_leaks("session").await, vec![]);
}

#[tokio::test]
async fn a_default_project_export_leaves_out_secrets_emails_paths_and_text_in_every_format() {
    assert_eq!(default_leaks("project").await, vec![]);
}

#[tokio::test]
async fn a_default_overview_export_leaves_out_secrets_emails_paths_and_text_in_every_format() {
    assert_eq!(default_leaks("overview").await, vec![]);
}

#[tokio::test]
async fn a_default_prompts_export_leaves_out_secrets_emails_paths_and_text_in_every_format() {
    assert_eq!(default_leaks("prompts").await, vec![]);
}

/// What an export with prompt text and raw payloads shows: (format, problem) for each format.
async fn with_text_problems(kind: &str) -> Vec<(String, String)> {
    let (api, repo) = data().await;
    let home = api.settings.user_home.display().to_string();
    let mut problems = Vec::new();
    for format in ["md", "html", "csv", "json"] {
        let body = fetch(&api, kind, &repo, &[("format", format), ("prompt_text", "true"), ("drop_raw", "false")]).await.text();
        let forbidden = NEVER.iter().map(|s| s.to_string()).chain([home.clone()]);
        problems.extend(forbidden.filter(|v| body.contains(v.as_str())).map(|v| (format.to_string(), format!("leaks {v}"))));
        let shown: &[&str] = match kind {
            "overview" => &[],                                                       // no prompt text
            "project" => &["deploy with [hidden AWS access key]", "[hidden email]"], // the start of each prompt only
            _ => &["deploy with [hidden AWS access key]", "[hidden email]", "src/main.py", "~/notes.txt"],
        };
        problems.extend(shown.iter().filter(|v| !body.contains(*v)).map(|v| (format.to_string(), format!("misses {v}"))));
    }
    problems
}

#[tokio::test]
async fn a_session_export_with_text_hides_secrets_emails_and_home_paths() {
    assert_eq!(with_text_problems("session").await, vec![]);
}

#[tokio::test]
async fn a_project_export_with_text_hides_secrets_emails_and_home_paths() {
    assert_eq!(with_text_problems("project").await, vec![]);
}

#[tokio::test]
async fn an_overview_export_with_text_hides_secrets_emails_and_home_paths() {
    assert_eq!(with_text_problems("overview").await, vec![]);
}

#[tokio::test]
async fn a_prompts_export_with_text_hides_secrets_emails_and_home_paths() {
    assert_eq!(with_text_problems("prompts").await, vec![]);
}

#[tokio::test]
async fn each_option_puts_back_what_it_hides_but_never_secrets() {
    let (api, repo) = data().await;
    let paths = fetch(&api, "session", &repo, &[("format", "json"), ("relative_paths", "false")]).await.text();
    assert!(paths.contains(&format!("{}/a.txt", repo.display())));
    let identity = fetch(&api, "session", &repo, &[("format", "md"), ("drop_identity", "false")]).await.text();
    assert!(identity.contains(ACCOUNT_EMAIL) && identity.contains(ORG_ID));
    let raw: Value = fetch(&api, "prompts", &repo, &[("format", "json"), ("drop_raw", "false"), ("prompt_text", "true")]).await.json();
    let rows = raw["rows"].as_array().expect("rows");
    assert_eq!(rows.last().expect("a row")["raw"]["hook_event_name"], "UserPromptSubmit");
    assert!(!raw.to_string().contains(AWS_KEY));
}

/// Tags and attributes that would make a browser fetch something.
fn external_resources(page: &str) -> Vec<String> {
    let lower = page.to_ascii_lowercase();
    let tags = ["script", "link", "img", "iframe", "object", "embed", "video", "audio", "source", "base"].map(|t| format!("<{t}"));
    let attributes = ["src=", "href=", "srcset=", "action=", "poster=", "data=", "xlink:href="].map(|a| format!(" {a}"));
    let text = ["http://", "https://", "url(", "@import"].map(String::from);
    tags.into_iter()
        .filter(|t| lower.contains(&format!("{t} ")) || lower.contains(&format!("{t}>")))
        .chain(attributes.into_iter().chain(text).filter(|a| lower.contains(a.as_str())))
        .collect()
}

async fn html_problems(kind: &str) -> Vec<String> {
    let (api, repo) = data().await;
    let response = fetch(&api, kind, &repo, &[("format", "html"), ("prompt_text", "true")]).await;
    let page = response.text();
    let mut problems = external_resources(&page);
    let csp = response.header("content-security-policy").unwrap_or_default();
    problems.extend((!csp.starts_with("default-src 'none'")).then(|| format!("CSP header {csp:?}")));
    let required = ["default-src 'none'", "<svg", "prefers-color-scheme: dark", "@media print"];
    problems.extend(required.iter().filter(|r| !page.contains(*r)).map(|r| format!("misses {r}")));
    problems
}

#[tokio::test]
async fn a_session_html_export_is_self_contained() {
    assert_eq!(html_problems("session").await, Vec::<String>::new());
}

#[tokio::test]
async fn a_project_html_export_is_self_contained() {
    assert_eq!(html_problems("project").await, Vec::<String>::new());
}

#[tokio::test]
async fn an_overview_html_export_is_self_contained() {
    assert_eq!(html_problems("overview").await, Vec::<String>::new());
}

#[tokio::test]
async fn a_prompts_html_export_is_self_contained() {
    assert_eq!(html_problems("prompts").await, Vec::<String>::new());
}

#[tokio::test]
async fn the_chart_list_names_the_charts_with_data_and_one_chart_downloads_as_svg() {
    let (api, repo) = data().await;
    let charts = api.json_with("/api/export/session/charts", &[("id", "s1")]).await;
    let ids: Vec<&str> = charts.as_array().expect("charts").iter().map(|c| c["id"].as_str().expect("id")).collect();
    assert!(ids.contains(&"outcomes") && ids.contains(&"tools"), "{ids:?}");
    let svg = fetch(&api, "session", &repo, &[("format", "svg"), ("chart", "tools")]).await;
    assert!(svg.header("content-type").is_some_and(|t| t.starts_with("image/svg+xml")));
    assert!(svg.text().contains("Tool calls by kind"));
}

#[tokio::test]
async fn one_chart_downloads_as_a_png_rendered_by_the_daemon() {
    let (api, repo) = data().await;
    let png = fetch(&api, "session", &repo, &[("format", "png"), ("chart", "tools")]).await;
    assert_eq!(png.status, 200);
    assert_eq!(png.header("content-type").as_deref(), Some("image/png"));
    assert!(png.body.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
}

#[tokio::test]
async fn an_unknown_chart_falls_back_to_the_first_one() {
    let (api, _) = data().await;
    let response = api.get("/api/export/session").query(&[("id", "s1"), ("format", "svg"), ("chart", "nope")]).send().await;
    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn an_export_is_shown_inline_never_cached_or_sniffed() {
    let (api, repo) = data().await;
    let response = fetch(&api, "session", &repo, &[("format", "md")]).await;
    assert_eq!(
        (response.header("cache-control"), response.header("x-content-type-options")),
        (Some("no-store".to_string()), Some("nosniff".to_string()))
    );
    assert!(response.header("content-disposition").is_some_and(|d| d.starts_with("inline; filename=\"gitprompt-session-")));
}

#[tokio::test]
async fn a_download_names_the_file_after_the_kind_title_and_the_clocks_day() {
    let (api, repo) = data().await;
    let response = fetch(&api, "project", &repo, &[("format", "md"), ("download", "true")]).await;
    let disposition = response.header("content-disposition").expect("disposition");
    assert!(disposition.starts_with("attachment; filename=\"gitprompt-project-app-"), "{disposition}");
    assert!(disposition.ends_with("-2026-03-04.md\""), "{disposition}");
}

#[tokio::test]
async fn export_errors_are_422_for_a_bad_request_and_404_for_an_unknown_id() {
    let (api, _) = data().await;
    let statuses = [
        api.get("/api/export/session").send().await.status, // no id
        api.get("/api/export/session?id=nope").send().await.status,
        api.get("/api/export/nothing").send().await.status,
        api.get("/api/export/session?id=s1&format=pdf").send().await.status,
    ];
    assert_eq!(statuses, [422, 404, 422, 422]);
}
