//! End to end, black box: the real `gitprompt` binary as a child process with an explicit
//! environment (temp GITPROMPT_HOME and HOME, a free loopback port), driven only through its
//! command line and HTTP over a plain `TcpStream` — what a tool's hook and the dashboard do.
//!
//! Covered: start-up and single instance, every connector fixture through `gitprompt hook`, every
//! read route the dashboard uses (and every export), the live-update stream, offline spooling, and
//! (unix) graceful shutdown on SIGINT. Every wait polls against a deadline.

mod common;

use std::io::{BufReader, Read};
use std::time::{Duration, Instant};

use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::binary::{COMMAND_DEADLINE, Fixture, Response, Served, connect, fixtures, read_head, send_head};
use common::{Home, free_port, home};

/// A Claude Code UserPromptSubmit in the documented shape.
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
    let (stdout, _) = served.output();
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
    let mut problems = Vec::new();
    let mut tools: Vec<&str> = cases.iter().map(|f| f.tool.as_str()).collect();
    tools.dedup();
    let data = served.json("/api/data");
    for tool in tools {
        let mine: Vec<&Fixture> = cases.iter().filter(|f| f.tool == tool).collect();
        let stored_as = |kind: &'static str| mine.iter().copied().filter(move |f| f.case["expect"]["stored_as"] == kind);

        let events: u64 =
            data["events"].as_array().expect("events").iter().filter(|s| s["source"] == tool).filter_map(|s| s["rows"].as_u64()).sum();
        let expected_events = stored_as("event").count() as u64;
        if events != expected_events {
            problems.push(format!("{tool}: {expected_events} events expected, {events} stored"));
        }

        let mut listed = served.json(&format!("/api/prompts?source={tool}&limit=200")).as_array().expect("prompt list").clone();
        listed.reverse(); // oldest first, the order the fixtures were delivered in
        let expected: Vec<&Fixture> = stored_as("prompt").collect();
        if listed.len() != expected.len() {
            problems.push(format!("{tool}: {} prompts expected, {} listed", expected.len(), listed.len()));
        }
        for (fixture, prompt) in expected.iter().zip(&listed) {
            let (name, expect) = (&fixture.name, &fixture.case["expect"]);
            if prompt["source"] != tool {
                problems.push(format!("{tool}/{name}: source {}", prompt["source"]));
            }
            if !is_subset(&expect["fields"], &prompt["fields"]) {
                problems.push(format!("{tool}/{name}: fields {}", prompt["fields"]));
            }
            if prompt["origin"] != expect["origin"] {
                problems.push(format!("{tool}/{name}: origin {}", prompt["origin"]));
            }
            if expect.get("typed_text").is_some() && prompt["typed_text"] != expect["typed_text"] {
                problems.push(format!("{tool}/{name}: typed_text {}", prompt["typed_text"]));
            }
        }
    }
    assert_eq!(problems, Vec::<String>::new());
    served.stop();
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

#[test]
fn every_read_route_the_dashboard_uses_answers_200_with_its_json_shape() {
    let home = home();
    let (served, _) = seeded(&home);
    let mut problems = Vec::new();
    let mut fetch = |path: &str| -> Value {
        let response = served.get(path);
        if response.status != 200 {
            problems.push(format!("{path}: {} {}", response.status, response.text()));
            return Value::Null;
        }
        if !response.header("content-type").is_some_and(|t| t.starts_with("application/json")) {
            problems.push(format!("{path}: content-type {:?}", response.header("content-type")));
        }
        response.json()
    };

    let prompts = fetch("/api/prompts?limit=50");
    let list = prompts.as_array().cloned().unwrap_or_default();
    let mut found: Vec<String> = Vec::new();
    if list.is_empty() || list.len() > 50 {
        found.push(format!("/api/prompts?limit=50: {} prompts", list.len()));
    }
    for prompt in &list {
        found.extend(shape("/api/prompts[]", prompt, &[("id", "number"), ("source", "string"), ("fields", "object"), ("facts", "object")]));
    }
    let claude = list.iter().find(|p| p["fields"]["session_id"] == "abc123").expect("a claude-code prompt of session abc123");
    let id = claude["id"].as_i64().expect("prompt id");

    let detail = fetch(&format!("/api/prompts/{id}"));
    found.extend(shape("/api/prompts/{id}", &detail, &[("id", "number"), ("raw", "object"), ("events", "array"), ("session", "object")]));
    found.extend(shape("/api/stats", &fetch("/api/stats"), &[("totals", "object"), ("groups", "array")]));
    for group_by in GROUP_BYS {
        let path = format!("/api/stats?group_by={group_by}");
        let stats = fetch(&path);
        found.extend(shape(&path, &stats, &[("group_by", "string"), ("totals", "object"), ("groups", "array")]));
        if stats["group_by"] != group_by {
            found.push(format!("{path}: group_by {}", stats["group_by"]));
        }
    }
    let telemetry = fetch("/api/stats/telemetry");
    found.extend(shape("/api/stats/telemetry", &telemetry, &[("sessions", "number"), ("metrics", "object"), ("commands", "array")]));

    let sources = fetch("/api/sources");
    let ids: Vec<&str> = sources.as_array().map(|s| s.iter().filter_map(|s| s["id"].as_str()).collect()).unwrap_or_default();
    for tool in ["claude-code", "codex", "copilot-cli", "cursor"] {
        if !ids.contains(&tool) {
            found.push(format!("/api/sources: no {tool} in {ids:?}"));
        }
    }
    for path in ["/api/sources/recent", "/api/sources/problems"] {
        let value = fetch(path);
        if !value.is_array() {
            found.push(format!("{path}: an array expected, got {}", kind(&value)));
        }
    }
    found.extend(shape("/api/sources/claude-code/spec", &fetch("/api/sources/claude-code/spec"), &[("file", "object")]));
    found.extend(shape("/api/fields", &fetch("/api/fields"), &[("fields", "array"), ("not_captured", "array"), ("derived", "object")]));
    found.extend(shape("/api/settings", &fetch("/api/settings"), &[("file", "string"), ("settings", "object")]));
    found.extend(shape(
        "/api/data",
        &fetch("/api/data"),
        &[("home", "string"), ("db_path", "string"), ("events", "array"), ("backups", "array")],
    ));
    found.extend(shape("/api/security", &fetch("/api/security"), &[("token_file", "string"), ("connected", "array")]));

    let story = fetch("/api/stories/session/abc123");
    found.extend(shape("/api/stories/session/abc123", &story, &[("session", "string"), ("project", "object")]));
    let project = story["project"]["id"].as_str().unwrap_or_default().to_owned();
    let path = format!("/api/stories/project/{project}");
    found.extend(shape(&path, &fetch(&path), &[("repo", "string"), ("totals", "object"), ("phases", "array")]));
    let lineage = fetch(&format!("/api/stories/prompt/{id}"));
    if !lineage.is_object() {
        found.push(format!("/api/stories/prompt/{id}: an object expected, got {}", kind(&lineage)));
    }
    let search = fetch("/api/search?q=retry");
    found.extend(shape("/api/search?q=retry", &search, &[("total", "number"), ("hits", "array")]));
    if search["total"].as_u64().unwrap_or_default() == 0 {
        found.push(format!("/api/search?q=retry: no hit for the codex prompt: {search}"));
    }
    let charts = fetch("/api/export/session/charts?id=abc123");
    if !charts.as_array().is_some_and(|c| !c.is_empty()) {
        found.push(format!("/api/export/session/charts: {charts}"));
    }

    // An unknown API path is a JSON 404, not the dashboard.
    let unknown = served.get("/api/nope");
    if unknown.status != 404 || unknown.json().get("detail").is_none() {
        found.push(format!("/api/nope: {} {}", unknown.status, unknown.text()));
    }
    // `/` is the dashboard when one is built or embedded, else a 404: only the status is stable.
    let root = served.get("/").status;
    if root != 200 && root != 404 {
        found.push(format!("/: {root}"));
    }

    problems.extend(found);
    assert_eq!(problems, Vec::<String>::new());
    served.stop();
}

#[test]
fn every_export_kind_renders_in_every_format_with_its_media_type() {
    let home = home();
    let (served, _) = seeded(&home);
    let project = served.json("/api/stories/session/abc123")["project"]["id"].as_str().expect("project id").to_owned();
    let kinds = [("session", Some("abc123".to_owned())), ("project", Some(project)), ("overview", None), ("prompts", None)];
    let formats = [
        ("md", "text/markdown"),
        ("html", "text/html"),
        ("csv", "text/csv"),
        ("json", "application/json"),
        ("svg", "image/svg+xml"),
        ("png", "image/png"),
    ];
    let mut problems = Vec::new();
    for (kind, id) in &kinds {
        for (format, media_type) in formats {
            let id = id.as_ref().map(|id| format!("&id={id}")).unwrap_or_default();
            let path = format!("/api/export/{kind}?format={format}{id}");
            let response: Response = served.get(&path);
            if response.status != 200 {
                problems.push(format!("{path}: {} {}", response.status, response.text()));
                continue;
            }
            if !response.header("content-type").is_some_and(|t| t.starts_with(media_type)) {
                problems.push(format!("{path}: content-type {:?}", response.header("content-type")));
            }
            let body_ok = match format {
                "png" => response.body.starts_with(b"\x89PNG\r\n\x1a\n"),
                "svg" => response.text().contains("<svg"),
                "json" => serde_json::from_slice::<Value>(&response.body).is_ok_and(|v| v.is_object() || v.is_array()),
                "html" => response.text().to_ascii_lowercase().contains("<html"),
                _ => !response.body.is_empty(),
            };
            if !body_ok {
                problems.push(format!("{path}: not a {format} body ({} bytes)", response.body.len()));
            }
        }
    }
    assert_eq!(problems, Vec::<String>::new());
    served.stop();
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
    first.stop();

    let before = Timestamp::now();
    let printed = deliver(&home, port, "claude-code", &typed("offline", "While stopped"));
    let after = Timestamp::now();
    assert_eq!(printed, "", "claude-code expects no reply on stdout");
    let queue = home.settings.spool_dir().join("claude-code.jsonl");
    let spooled = std::fs::read_to_string(&queue).unwrap_or_else(|e| panic!("{}: {e}", queue.display()));
    assert_eq!(spooled.lines().count(), 1, "one payload spooled: {spooled}");

    let second = home.serve(); // drains the spool before it answers /healthz
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
