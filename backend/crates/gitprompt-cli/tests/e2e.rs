//! End to end, black box: the real `gitprompt` binary as a child process with an explicit
//! environment (temp `GITPROMPT_HOME` and HOME, a free loopback port), driven only through its
//! command line and HTTP over a plain `TcpStream` — what a tool's hook and the dashboard do.
//!
//! Covered: start-up and single instance, every connector fixture through `gitprompt hook`, every
//! read route the dashboard uses (and every export), the live-update stream, offline spooling, and
//! (unix) graceful shutdown on SIGINT. Every wait polls against a deadline.
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

use std::io::{BufReader, Read};
use std::time::{Duration, Instant};

use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::binary::{COMMAND_DEADLINE, Fixture, Response, Served, connect, fixtures, read_head, send_head};
use common::{Home, free_port, home};

/// A Claude Code `UserPromptSubmit` in the documented shape.
fn typed(session: &str, text: &str) -> Value {
    json!({
        "session_id": session,
        "prompt_id": format!("{session}-{text}"),
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": "/work/e2e",
        "permission_mode": "default",
        "hook_event_name": "UserPromptSubmit",
        "prompt": text,
    })
}

/// `gitprompt hook <source>` must exit 0 and never write on stderr (a tool shows that as a hook
/// error); stdout is only the reply the tool expects.
fn deliver(home: &Home, port: u16, source: &str, payload: &Value) -> String {
    let output = home.hook(port, source, payload);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() && stderr.is_empty(), "hook {source}: {} {stderr}", output.status);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A daemon holding every fixture of every tool, delivered through the hook command.
fn seeded(home: &Home) -> (Served, Vec<Fixture>) {
    let served = home.serve();
    let cases = fixtures();
    assert!(!cases.is_empty(), "no fixtures found");
    for fixture in &cases {
        deliver(home, served.port, &fixture.tool, &fixture.case["payload"]);
    }
    (served, cases)
}

fn is_subset(expected: &Value, actual: &Value) -> bool {
    expected.as_object().expect("expected fields").iter().all(|(k, v)| actual.get(k) == Some(v))
}

// --- (a) start-up and single instance ---

#[test]
fn serve_answers_healthz_and_a_second_serve_on_the_same_home_fails_fast_naming_the_lock() {
    let home = home();
    let served = home.serve();
    assert_eq!(served.json("/healthz"), json!({"ok": true}));
    let (stdout, _) = served.output(); // complete: `serve()` returns once it printed
    assert!(stdout.contains(&format!("127.0.0.1:{}", served.port)), "serve should say where it runs: {stdout:?}");

    let started = Instant::now();
    let second = home.run(free_port(), &["serve"], b"");
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert_eq!(second.status.code(), Some(1), "second serve: {stderr}");
    assert!(started.elapsed() < Duration::from_secs(5), "the refusal took {:?}", started.elapsed());
    assert!(stderr.contains("already running"), "unclear refusal: {stderr:?}");
    assert!(stderr.contains(&home.settings.lock_file().display().to_string()), "the refusal should name the lock: {stderr:?}");

    assert_eq!(served.json("/healthz"), json!({"ok": true}), "the first daemon keeps running");
    served.stop();
}

// --- (b) every connector fixture, through the hook command ---

#[test]
fn every_fixture_delivered_through_the_hook_command_is_stored_as_its_expectation_says() {
    let home = home();
    let (served, cases) = seeded(&home);
    let data = served.json("/api/data");
    let mut problems = Vec::new();
    // `fixtures()` lists them tool by tool, so each chunk is one tool's.
    for mine in cases.chunk_by(|a, b| a.tool == b.tool) {
        let tool = mine[0].tool.as_str();
        problems.extend(count_problem(tool, "events", stored_as(mine, "event").count(), events_stored(&data, tool)));
        let listed = prompts_listed(&served, tool);
        problems.extend(count_problem(tool, "prompts", stored_as(mine, "prompt").count(), listed.len()));
        for (fixture, prompt) in stored_as(mine, "prompt").zip(&listed) {
            problems.extend(prompt_problems(fixture, prompt));
        }
    }
    assert_eq!(problems, Vec::<String>::new());
    served.stop();
}

/// The fixtures whose expectation says they are stored as `kind` (`prompt` or `event`).
fn stored_as<'a>(fixtures: &'a [Fixture], kind: &'a str) -> impl Iterator<Item = &'a Fixture> {
    fixtures.iter().filter(move |f| f.case["expect"]["stored_as"] == kind)
}

/// The events `/api/data` holds for `tool`.
fn events_stored(data: &Value, tool: &str) -> usize {
    let events = data["events"].as_array().expect("events");
    events.iter().filter(|s| s["source"] == tool).filter_map(|s| s["rows"].as_u64()).sum::<u64>() as usize
}

/// `tool`'s prompts as listed, oldest first: the order the fixtures were delivered in.
fn prompts_listed(served: &Served, tool: &str) -> Vec<Value> {
    let mut listed = served.json(&format!("/api/prompts?source={tool}&limit=200")).as_array().expect("prompt list").clone();
    listed.reverse();
    listed
}

/// A problem unless as many `what` were found as expected.
fn count_problem(tool: &str, what: &str, expected: usize, found: usize) -> Option<String> {
    (found != expected).then(|| format!("{tool}: {expected} {what} expected, {found} found"))
}

/// Each field of a stored prompt that differs from its fixture's expectation (`typed_text` only
/// when the fixture expects one).
fn prompt_problems(fixture: &Fixture, prompt: &Value) -> Vec<String> {
    let expect = &fixture.case["expect"];
    let checks = [
        ("source", prompt["source"] == fixture.tool.as_str()),
        ("fields", is_subset(&expect["fields"], &prompt["fields"])),
        ("origin", prompt["origin"] == expect["origin"]),
        ("typed_text", expect.get("typed_text").is_none() || prompt["typed_text"] == expect["typed_text"]),
    ];
    checks
        .into_iter()
        .filter(|(_, ok)| !ok)
        .map(|(field, _)| format!("{}/{}: {field} {}", fixture.tool, fixture.name, prompt[field]))
        .collect()
}

// --- (c) every read route the dashboard uses ---

/// The JSON kind of a value, for shape assertions.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// `(path, problem)` unless `value` is an object with each of `keys` of the given kind.
fn shape(path: &str, value: &Value, keys: &[(&str, &str)]) -> Vec<String> {
    if !value.is_object() {
        return vec![format!("{path}: an object expected, got {}", kind(value))];
    }
    keys.iter()
        .filter(|(key, want)| kind(&value[*key]) != *want)
        .map(|(key, want)| format!("{path}: .{key} should be {want}, is {}", kind(&value[*key])))
        .collect()
}

/// Every `group_by` value of `/api/stats` (`gitprompt_store::stats::GroupBy`).
const GROUP_BYS: [&str; 21] = [
    "day",
    "day_hour",
    "hour",
    "weekday",
    "weekday_hour",
    "origin",
    "source",
    "surface",
    "session",
    "session_length",
    "repo",
    "branch",
    "command",
    "size",
    "text",
    "outcome",
    "model",
    "tool",
    "tool_kind",
    "work",
    "query_source",
];

/// The read routes of one served daemon, checked one after another; every problem is collected so
/// a single run names all of them.
struct RouteCheck<'a> {
    served: &'a Served,
    problems: Vec<String>,
}

impl RouteCheck<'_> {
    /// `GET path` as JSON: `null` (and a problem) unless it answers 200 with a JSON content type.
    fn fetch(&mut self, path: &str) -> Value {
        let response = self.served.get(path);
        if response.status != 200 {
            self.problem(format!("{path}: {} {}", response.status, response.text()));
            return Value::Null;
        }
        if !response.header("content-type").is_some_and(|t| t.starts_with("application/json")) {
            self.problem(format!("{path}: content-type {:?}", response.header("content-type")));
        }
        response.json()
    }

    /// [`Self::fetch`] `path`, which must be an object with each of `keys` of the given kind.
    fn object(&mut self, path: &str, keys: &[(&str, &str)]) -> Value {
        let value = self.fetch(path);
        self.shape(path, &value, keys);
        value
    }

    /// [`Self::fetch`] `path`, which must be an array.
    fn array(&mut self, path: &str) -> Value {
        let value = self.fetch(path);
        if !value.is_array() {
            self.problem(format!("{path}: an array expected, got {}", kind(&value)));
        }
        value
    }

    fn shape(&mut self, path: &str, value: &Value, keys: &[(&str, &str)]) {
        self.problems.extend(shape(path, value, keys));
    }

    fn problem(&mut self, problem: String) {
        self.problems.push(problem);
    }
}

/// The prompt list and one prompt's detail; the id of the claude-code prompt of session `abc123`.
fn check_prompts(check: &mut RouteCheck<'_>) -> i64 {
    let list = check.array("/api/prompts?limit=50").as_array().cloned().unwrap_or_default();
    if list.is_empty() || list.len() > 50 {
        check.problem(format!("/api/prompts?limit=50: {} prompts", list.len()));
    }
    for prompt in &list {
        check.shape("/api/prompts[]", prompt, &[("id", "number"), ("source", "string"), ("fields", "object"), ("facts", "object")]);
    }
    let claude = list.iter().find(|p| p["fields"]["session_id"] == "abc123").expect("a claude-code prompt of session abc123");
    let id = claude["id"].as_i64().expect("prompt id");
    check.object(&format!("/api/prompts/{id}"), &[("id", "number"), ("raw", "object"), ("events", "array"), ("session", "object")]);
    id
}

/// `/api/stats` ungrouped, by every `group_by`, and its telemetry.
fn check_stats(check: &mut RouteCheck<'_>) {
    check.object("/api/stats", &[("totals", "object"), ("groups", "array")]);
    for group_by in GROUP_BYS {
        let path = format!("/api/stats?group_by={group_by}");
        let stats = check.object(&path, &[("group_by", "string"), ("totals", "object"), ("groups", "array")]);
        if stats["group_by"] != group_by {
            check.problem(format!("{path}: group_by {}", stats["group_by"]));
        }
    }
    check.object("/api/stats/telemetry", &[("sessions", "number"), ("metrics", "object"), ("commands", "array")]);
}

/// The Connectors page: every bundled tool listed, recent deliveries, problems, a spec.
fn check_sources(check: &mut RouteCheck<'_>) {
    let sources = check.array("/api/sources");
    let ids: Vec<&str> = sources.as_array().map(|s| s.iter().filter_map(|s| s["id"].as_str()).collect()).unwrap_or_default();
    for tool in ["claude-code", "codex", "copilot-cli", "cursor"].into_iter().filter(|tool| !ids.contains(tool)) {
        check.problem(format!("/api/sources: no {tool} in {ids:?}"));
    }
    check.array("/api/sources/recent");
    check.array("/api/sources/problems");
    check.object("/api/sources/claude-code/spec", &[("file", "object")]);
}

/// Fields, settings, data and security pages.
fn check_settings(check: &mut RouteCheck<'_>) {
    check.object("/api/fields", &[("fields", "array"), ("not_captured", "array"), ("derived", "object")]);
    check.object("/api/settings", &[("file", "string"), ("settings", "object")]);
    check.object("/api/data", &[("home", "string"), ("db_path", "string"), ("events", "array"), ("backups", "array")]);
    check.object("/api/security", &[("token_file", "string"), ("connected", "array")]);
}

/// The session story, its project's story and prompt `id`'s lineage.
fn check_stories(check: &mut RouteCheck<'_>, id: i64) {
    let story = check.object("/api/stories/session/abc123", &[("session", "string"), ("project", "object")]);
    let project = story["project"]["id"].as_str().unwrap_or_default();
    check.object(&format!("/api/stories/project/{project}"), &[("repo", "string"), ("totals", "object"), ("phases", "array")]);
    let path = format!("/api/stories/prompt/{id}");
    let lineage = check.fetch(&path);
    if !lineage.is_object() {
        check.problem(format!("{path}: an object expected, got {}", kind(&lineage)));
    }
}

/// Search finds the codex prompt; the session's charts export is not empty.
fn check_search_and_charts(check: &mut RouteCheck<'_>) {
    let search = check.object("/api/search?q=retry", &[("total", "number"), ("hits", "array")]);
    if search["total"].as_u64().unwrap_or_default() == 0 {
        check.problem(format!("/api/search?q=retry: no hit for the codex prompt: {search}"));
    }
    let charts = check.fetch("/api/export/session/charts?id=abc123");
    if charts.as_array().is_none_or(Vec::is_empty) {
        check.problem(format!("/api/export/session/charts: {charts}"));
    }
}

/// An unknown API path is a JSON 404, not the dashboard; `/` is the dashboard when one is built
/// or embedded, else a 404 (only the status is stable).
fn check_fallbacks(check: &mut RouteCheck<'_>) {
    let unknown = check.served.get("/api/nope");
    if unknown.status != 404 || unknown.json().get("detail").is_none() {
        check.problem(format!("/api/nope: {} {}", unknown.status, unknown.text()));
    }
    let root = check.served.get("/").status;
    if root != 200 && root != 404 {
        check.problem(format!("/: {root}"));
    }
}

#[test]
fn every_read_route_the_dashboard_uses_answers_200_with_its_json_shape() {
    let home = home();
    let (served, _) = seeded(&home);
    let mut check = RouteCheck { served: &served, problems: Vec::new() };
    let id = check_prompts(&mut check);
    check_stats(&mut check);
    check_sources(&mut check);
    check_settings(&mut check);
    check_stories(&mut check, id);
    check_search_and_charts(&mut check);
    check_fallbacks(&mut check);
    assert_eq!(check.problems, Vec::<String>::new());
    served.stop();
}

/// Every export format with the media type it is served as.
const EXPORT_FORMATS: [(&str, &str); 6] = [
    ("md", "text/markdown"),
    ("html", "text/html"),
    ("csv", "text/csv"),
    ("json", "application/json"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
];

#[test]
fn every_export_kind_renders_in_every_format_with_its_media_type() {
    let home = home();
    let (served, _) = seeded(&home);
    let project = served.json("/api/stories/session/abc123")["project"]["id"].as_str().expect("project id").to_owned();
    // Each kind with the query its id adds.
    let kinds = [
        ("session", "&id=abc123".to_owned()),
        ("project", format!("&id={project}")),
        ("overview", String::new()),
        ("prompts", String::new()),
    ];
    let paths = kinds.iter().flat_map(|(kind, id)| {
        EXPORT_FORMATS.iter().map(move |&(format, media_type)| (format!("/api/export/{kind}?format={format}{id}"), format, media_type))
    });
    let problems: Vec<String> = paths.flat_map(|(path, format, media_type)| export_problems(&served, &path, format, media_type)).collect();
    assert_eq!(problems, Vec::<String>::new());
    served.stop();
}

/// What is wrong with the export at `path`: not a 200, or else a content type other than
/// `media_type` and a body that is not a `format` document.
fn export_problems(served: &Served, path: &str, format: &str, media_type: &str) -> Vec<String> {
    let response: Response = served.get(path);
    if response.status != 200 {
        return vec![format!("{path}: {} {}", response.status, response.text())];
    }
    let mut problems = Vec::new();
    let content_type = response.header("content-type");
    if !content_type.is_some_and(|t| t.starts_with(media_type)) {
        problems.push(format!("{path}: content-type {content_type:?}"));
    }
    if !is_document(format, &response) {
        problems.push(format!("{path}: not a {format} body ({} bytes)", response.body.len()));
    }
    problems
}

/// Whether the body looks like a `format` document (its signature, or at least not empty).
fn is_document(format: &str, response: &Response) -> bool {
    match format {
        "png" => response.body.starts_with(b"\x89PNG\r\n\x1a\n"),
        "svg" => response.text().contains("<svg"),
        "json" => serde_json::from_slice::<Value>(&response.body).is_ok_and(|v| v.is_object() || v.is_array()),
        "html" => response.text().to_ascii_lowercase().contains("<html"),
        _ => !response.body.is_empty(),
    }
}

// --- (d) the live-update stream ---

#[test]
fn the_live_stream_emits_a_notice_after_a_hook_delivers_a_prompt() {
    let home = home();
    let served = home.serve();
    let mut stream = connect(served.port).expect("connect");
    send_head(&mut stream, served.port, "GET", "/api/prompts/stream", &[("Accept", "text/event-stream")], 0).expect("request");
    let mut reader = BufReader::new(stream);
    // The route subscribes before it answers, so once the head is read no notice can be missed.
    let (status, headers) = read_head(&mut reader).expect("response head");
    assert_eq!(status, 200);
    let content_type = headers.iter().find(|(n, _)| n == "content-type").map(|(_, v)| v.as_str());
    assert!(content_type.is_some_and(|t| t.starts_with("text/event-stream")), "content-type {content_type:?}");

    deliver(&home, served.port, "claude-code", &typed("live", "Stream me"));

    let deadline = Instant::now() + COMMAND_DEADLINE;
    let mut received = Vec::new();
    let notice = loop {
        let text = String::from_utf8_lossy(&received).into_owned();
        if let Some(line) = text.lines().find_map(|l| l.strip_prefix("data: ")).filter(|_| text.contains("}\n")) {
            break serde_json::from_str::<Value>(line.trim()).expect("notice JSON");
        }
        assert!(Instant::now() < deadline, "no notice within {COMMAND_DEADLINE:?}; got {text:?}");
        let mut chunk = [0; 1024];
        let n = reader.read(&mut chunk).expect("read the stream");
        assert!(n > 0, "the stream ended before a notice; got {text:?}");
        received.extend_from_slice(&chunk[..n]);
    };
    assert_eq!(notice, json!({"prompts": 1, "events": 0}));
    drop(reader);
    served.stop();
}

// --- (e) offline spooling ---

#[test]
fn a_hook_sent_while_the_daemon_is_stopped_is_spooled_silently_and_stored_at_its_original_time_by_the_next_serve() {
    let home = home();
    let first = home.serve();
    let port = first.port;
    deliver(&home, port, "claude-code", &typed("offline", "Before the stop"));
    // The live daemon stored it. On a loaded machine (a CI runner running the other tests' daemons)
    // its answer can come after the forwarder's 2 s timeout, and the forwarder spools that payload
    // too: allowed, since the prompt-id dedup absorbs it at the next drain (see spool.rs), and the
    // final list below still fails on a lost or duplicated prompt.
    common::binary::eventually(COMMAND_DEADLINE, "the running daemon stores the prompt", || {
        (first.json("/api/prompts?session=offline").as_array().map(Vec::len) == Some(1)).then_some(())
    });
    first.stop();

    let before = Timestamp::now();
    let printed = deliver(&home, port, "claude-code", &typed("offline", "While stopped"));
    let after = Timestamp::now();
    assert_eq!(printed, "", "claude-code expects no reply on stdout");
    let queue = home.settings.spool_dir().join("claude-code.jsonl");
    let spooled = std::fs::read_to_string(&queue).unwrap_or_else(|e| panic!("{}: {e}", queue.display()));
    let prompts_in = |line: &str| serde_json::from_str::<Value>(line).ok().and_then(|l| l["payload"]["prompt"].as_str().map(str::to_owned));
    let queued: Vec<String> = spooled.lines().filter_map(prompts_in).collect();
    assert_eq!(queued.last().map(String::as_str), Some("While stopped"), "the stopped daemon's payload is spooled: {spooled}");
    assert!(queued.iter().all(|p| p == "While stopped" || p == "Before the stop"), "only this test's payloads: {spooled}");

    let second = home.serve(); // answers at once and drains the spool behind, holding its lock
    // `spool drain` waits on that lock until the start-up drain is over, and finds nothing left.
    home.cmd_on(second.port, &["spool", "drain"]).assert().success().stdout("Delivered 0 spooled payloads, 0 left.\n");
    let prompts = second.json("/api/prompts?session=offline");
    let texts: Vec<&str> = prompts.as_array().expect("list").iter().filter_map(|p| p["prompt"].as_str()).collect();
    assert_eq!(texts, ["While stopped", "Before the stop"]);
    let received_at: Timestamp = prompts[0]["received_at"].as_str().expect("received_at").parse().expect("an instant");
    assert!(before <= received_at && received_at <= after, "stored at {received_at}, sent between {before} and {after}");
    let left = std::fs::read_to_string(&queue).unwrap_or_default();
    assert_eq!(left.trim(), "", "the spool is emptied once delivered");
    second.stop();
}

// --- (f) graceful shutdown (unix: SIGINT) ---

#[cfg(unix)]
#[test]
fn sigint_stops_serve_within_two_seconds_and_releases_the_lock() {
    let home = home();
    let mut served = home.serve();
    deliver(&home, served.port, "claude-code", &typed("sigint", "Keep me"));

    let sent = Instant::now();
    served.interrupt();
    let status = served.wait_exit(Duration::from_secs(2));
    let elapsed = sent.elapsed();
    let (_, stderr) = served.output();
    let status = status.unwrap_or_else(|| panic!("still running {elapsed:?} after SIGINT; stderr: {stderr}"));
    assert!(status.success(), "exit {status} after SIGINT; stderr: {stderr}");

    // The lock is free: a new daemon starts on the same home, and nothing was lost.
    let again = home.serve();
    let prompts = again.json("/api/prompts?session=sigint");
    assert_eq!(prompts.as_array().map(Vec::len), Some(1), "{prompts}");
    again.stop();
}
