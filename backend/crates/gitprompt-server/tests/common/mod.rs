//! Shared test harness: an app in a temp dir driven through its axum router (no sockets), with a
//! small request builder that sets the Host header and the auth headers.
//!
//! Everything lives under one canonicalized temp dir: `<tmp>/gitprompt` is the data directory and
//! `<tmp>/home` stands in for `~` (tests never touch the real ~/.claude, ~/.codex...). The
//! environment the specs and preferences read is `Settings::env`, set per test — never the process
//! environment.
#![allow(dead_code, reason = "each test binary uses a different subset of these helpers")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request};
use http_body_util::BodyExt;
use jiff::Timestamp;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use gitprompt_core::Settings;
use gitprompt_server::{App, AppOptions, Dashboard, FixedClock, FixedVersion, Service, SystemGit};

pub const HOST: &str = "127.0.0.1:7878";
/// The version every tool reports in these tests (the Python suite's `tool_version` stub).
pub const TOOL_VERSION: &str = "2.1.316";

// --- building an app ---

pub struct Harness {
    pub tmp: TempDir,
    pub settings: Settings,
    pub options: AppOptions,
}

/// An app with the defaults below; see [`harness`] to change them first.
pub fn api() -> Api {
    harness().start()
}

/// Settings rooted in a fresh temp dir with an empty environment; every tool at [`TOOL_VERSION`];
/// the real git CLI (temp repos only); the system clock; no dashboard.
pub fn harness() -> Harness {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().canonicalize().expect("canonical temp dir");
    let settings = Settings::new(root.join("gitprompt"), root.join("home"));
    let options = AppOptions {
        tool_version: Arc::new(FixedVersion(Some(TOOL_VERSION.into()))),
        git: Arc::new(SystemGit::new(std::time::Duration::ZERO)),
        ..AppOptions::default()
    };
    Harness { tmp, settings, options }
}

impl Harness {
    /// A variable the specs and preferences see (`CODEX_HOME`, `GITPROMPT_DISABLED_SOURCES`...).
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.settings.env.set(name, value);
        self
    }

    pub fn clock(mut self, now: Timestamp) -> Self {
        self.options.clock = Arc::new(FixedClock(now));
        self
    }

    pub fn dashboard(mut self, dir: PathBuf) -> Self {
        self.options.dashboard = Some(Dashboard::Dir(dir));
        self
    }

    /// Hooks installed in the native forwarder form, running `path` (the desktop app's sidecar).
    pub fn forwarder(mut self, path: PathBuf) -> Self {
        self.options.forwarder = Some(path);
        self
    }

    pub fn applies_at_start(mut self, keys: &[&str]) -> Self {
        self.options.applies_at_start = keys.iter().map(std::string::ToString::to_string).collect();
        self
    }

    pub fn root(&self) -> PathBuf {
        self.settings.user_home.parent().expect("root").to_path_buf()
    }

    pub fn start(self) -> Api {
        let app = App::new(self.settings.clone(), self.options).expect("app starts");
        let router = app.router();
        Api { settings: self.settings, router, app, _tmp: self.tmp }
    }
}

pub struct Api {
    pub settings: Settings,
    pub router: Router,
    pub app: App,
    _tmp: TempDir,
}

impl Api {
    /// The temp root holding `gitprompt/` and `home/`.
    pub fn root(&self) -> PathBuf {
        self.settings.user_home.parent().expect("root").to_path_buf()
    }

    pub fn service(&self) -> Service {
        self.app.service()
    }

    pub fn token(&self) -> String {
        self.settings.token().expect("token")
    }

    pub fn get(&self, path: &str) -> Req<'_> {
        Req::new(self, "GET", path)
    }

    pub fn post(&self, path: &str) -> Req<'_> {
        Req::new(self, "POST", path)
    }

    pub fn put(&self, path: &str) -> Req<'_> {
        Req::new(self, "PUT", path)
    }

    /// POST a hook payload with the token; the receiver must answer an empty 204.
    pub async fn ingest(&self, source: &str, payload: Value) {
        let response = self.post(&format!("/ingest/{source}")).auth().json(payload).send().await;
        assert_eq!((response.status, response.text()), (204, String::new()), "ingest into {source}");
    }

    /// POST an OTLP logs export with the token; the receiver must answer `{}`.
    pub async fn ingest_otlp(&self, source: &str, payload: Value) {
        let response = self.post(&format!("/ingest/{source}/v1/logs")).auth().json(payload).send().await;
        assert_eq!((response.status, response.json()), (200, json!({})), "OTLP into {source}");
    }

    /// GET that must succeed; its JSON body.
    pub async fn json(&self, path: &str) -> Value {
        let response = self.get(path).send().await;
        assert_eq!(response.status, 200, "GET {path}: {}", response.text());
        response.json()
    }

    /// GET with query pairs that must succeed; its JSON body.
    pub async fn json_with(&self, path: &str, query: &[(&str, &str)]) -> Value {
        let response = self.get(path).query(query).send().await;
        assert_eq!(response.status, 200, "GET {path} {query:?}: {}", response.text());
        response.json()
    }

    /// One connector of `GET /api/sources`.
    pub async fn source(&self, id: &str) -> Value {
        let sources = self.json("/api/sources").await;
        sources.as_array().expect("list").iter().find(|s| s["id"] == id).cloned().unwrap_or_else(|| panic!("no source {id}"))
    }

    /// Run SQL on the app's own database file (WAL: safe beside the open store), through a
    /// connection of its own. For moving `received_at` into the past after ingest, which the
    /// pipeline (rightly) never does; the Python suite does the same through `store.db`.
    pub fn sql(&self, statement: &str) {
        sql(&self.settings.db_path(), statement);
    }
}

pub fn sql(db: &Path, statement: &str) {
    let conn = rusqlite::Connection::open(db).expect("open the database");
    conn.busy_timeout(std::time::Duration::from_secs(5)).expect("busy timeout");
    conn.execute_batch(statement).unwrap_or_else(|e| panic!("{statement}: {e}"));
}

// --- requests ---

pub struct Req<'a> {
    api: &'a Api,
    method: &'static str,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

impl<'a> Req<'a> {
    fn new(api: &'a Api, method: &'static str, path: &str) -> Self {
        Req { api, method, path: path.to_string(), headers: vec![("host".into(), HOST.into())], body: None }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        let name = name.to_ascii_lowercase();
        self.headers.retain(|(n, _)| *n != name);
        self.headers.push((name, value.to_string()));
        self
    }

    pub fn without(mut self, name: &str) -> Self {
        let name = name.to_ascii_lowercase();
        self.headers.retain(|(n, _)| *n != name);
        self
    }

    pub fn host(self, host: &str) -> Self {
        self.header("host", host)
    }

    /// `X-GitPrompt: 1`, which every write needs.
    pub fn dashboard(self) -> Self {
        self.header("x-gitprompt", "1")
    }

    /// `Authorization: Bearer <current token>`.
    pub fn auth(self) -> Self {
        let token = self.api.token();
        self.bearer(&token)
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header("authorization", &format!("Bearer {token}"))
    }

    pub fn json(mut self, body: Value) -> Self {
        self.body = Some(serde_json::to_vec(&body).expect("json"));
        self.header("content-type", "application/json")
    }

    /// Append query pairs (repeated keys allowed), percent-encoded.
    pub fn query(mut self, pairs: &[(&str, &str)]) -> Self {
        let encoded: Vec<String> = pairs.iter().map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect();
        let separator = if self.path.contains('?') { '&' } else { '?' };
        self.path = format!("{}{separator}{}", self.path, encoded.join("&"));
        self
    }

    pub async fn send(self) -> Resp {
        let mut request = Request::builder().method(self.method).uri(&self.path);
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let request = request.body(self.body.map_or_else(Body::empty, Body::from)).expect("request");
        let response = self.api.router.clone().oneshot(request).await.expect("infallible router");
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.expect("body").to_bytes().to_vec();
        Resp { status, headers, body }
    }
}

pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub struct Resp {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.text()))
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.get(name).map(|v| v.to_str().expect("ascii header").to_string())
    }
}

// --- payloads (synthetic, in the documented shapes) ---

/// Claude Code `UserPromptSubmit`, as documented at <https://code.claude.com/docs/en/hooks>.
pub fn hook_payload(cwd: &Path) -> Value {
    json!({
        "session_id": "abc123",
        "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": cwd.to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "UserPromptSubmit",
        "prompt": "Write a function to calculate the factorial of a number",
    })
}

/// `base` with `changes` merged in at the top level (Python's `payload | {...}`).
pub fn with(base: &Value, changes: Value) -> Value {
    let mut out = base.clone();
    let target = out.as_object_mut().expect("object");
    for (k, v) in changes.as_object().expect("object") {
        target.insert(k.clone(), v.clone());
    }
    out
}

/// An OTLP/HTTP JSON logs export with one record: `attributes` and `resource` as string values
/// unless given as OTLP `{"intValue"...}` objects.
pub fn otlp(resource: Value, attributes: Value) -> Value {
    fn attrs(values: &Value) -> Value {
        let list: Vec<Value> = values
            .as_object()
            .expect("object")
            .iter()
            .map(|(k, v)| match v {
                Value::String(s) => json!({"key": k, "value": {"stringValue": s}}),
                other => json!({"key": k, "value": other}),
            })
            .collect();
        Value::Array(list)
    }
    json!({"resourceLogs": [{
        "resource": {"attributes": attrs(&resource)},
        "scopeLogs": [{"logRecords": [{"body": {"stringValue": "claude_code.event"}, "attributes": attrs(&attributes)}]}],
    }]})
}

/// The number at `value` (JSON ints and floats compare as numbers).
pub fn num(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| panic!("not a number: {value}"))
}

/// `{key: value[measure]}` of a stats response's groups, keys as strings (`null` -> "null").
pub fn groups(stats: &Value, measure: &str) -> std::collections::BTreeMap<String, f64> {
    stats["groups"].as_array().expect("groups").iter().map(|g| (key(&g["key"]), num(&g[measure]))).collect()
}

pub fn key(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The dashboard's project id: base64url of the repository path, no padding (`stories.project_id`).
pub fn project_id(repo: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = repo.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        let chars = chunk.len() + 1;
        out.extend((0..chars).map(|i| ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char));
    }
    out
}

// --- git (temp repositories only; the user's git config is ignored) ---

pub fn git(repo: &Path, args: &[&str], when: Option<&str>) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    if let Some(when) = when {
        command.env("GIT_COMMITTER_DATE", when).env("GIT_AUTHOR_DATE", when);
    }
    let output = command.output().expect("git");
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

/// A repository at `path` with one commit of `a.txt`.
pub fn repo(path: &Path, when: Option<&str>) -> PathBuf {
    std::fs::create_dir_all(path).expect("mkdir");
    git(path, &["init", "-b", "main"], None);
    std::fs::write(path.join("a.txt"), "a\n").expect("write");
    git(path, &["add", "."], None);
    git(path, &["commit", "-m", "initial"], when);
    path.to_path_buf()
}

/// `<user_home>/.claude/settings.json`, its folder created (the tool is "installed").
pub fn claude_settings(settings: &Settings) -> PathBuf {
    let path = settings.user_home.join(".claude").join("settings.json");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    path
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}
