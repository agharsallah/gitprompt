//! The plugin in a mock Tauri app: setup starts the daemon, commands answer over IPC, notices
//! become events, an external daemon or a taken port leave the app usable, shutdown releases the
//! lock.
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

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::utils::acl::ExecutionContext;
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

use gitprompt_core::{Env, Settings};
use gitprompt_server::{AppOptions, Daemon, DaemonOptions, FixedVersion, NoGit, Notice};
use gitprompt_tauri::{CAPTURED_EVENT, COMMANDS, DaemonStatus, GitPromptExt, Opener, PluginConfig};

/// Records what the commands asked to show.
#[derive(Default)]
struct Recorder {
    opened: Mutex<Vec<PathBuf>>,
    revealed: Mutex<Vec<PathBuf>>,
}

impl Opener for Recorder {
    fn open(&self, path: &Path) -> std::io::Result<()> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
    fn reveal(&self, path: &Path) -> std::io::Result<()> {
        self.revealed.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

struct Harness {
    root: TempDir,
    recorder: Arc<Recorder>,
    port: u16,
}

impl Harness {
    fn new() -> Self {
        Harness { root: tempfile::tempdir().unwrap(), recorder: Arc::default(), port: 0 }
    }

    fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("gitprompt")
    }

    fn user_home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn settings(&self) -> Settings {
        Settings::new(self.home(), self.user_home())
    }

    fn options() -> AppOptions {
        AppOptions { tool_version: Arc::new(FixedVersion(None)), git: Arc::new(NoGit), ..AppOptions::default() }
    }

    fn config(&self) -> PluginConfig {
        PluginConfig {
            env: Some(Env::from_pairs([("HOME", self.user_home().display().to_string())])),
            data_dir: Some(self.home()),
            port: Some(self.port),
            serve_dashboard: false,
            options: Some(Self::options()),
            opener: Some(self.recorder.clone()),
            ..PluginConfig::default()
        }
    }

    /// The mock app with the plugin, every plugin command allowed for local webviews, the start
    /// settled.
    fn app(&self) -> App<MockRuntime> {
        let mut context = mock_context(noop_assets());
        for command in COMMANDS {
            context.runtime_authority_mut().__allow_command(format!("plugin:gitprompt|{command}"), ExecutionContext::Local);
        }
        let app = mock_builder().plugin(gitprompt_tauri::init(self.config())).build(context).unwrap();
        tauri::async_runtime::block_on(app.gitprompt().unwrap().ready());
        app
    }
}

fn webview(app: &App<MockRuntime>) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, "main", WebviewUrl::default()).build().unwrap()
}

/// Invoke `plugin:gitprompt|<command>`: the answer, or the error value.
fn invoke(webview: &WebviewWindow<MockRuntime>, command: &str, args: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: format!("plugin:gitprompt|{command}"),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(webview, request).map(|body| body.deserialize::<Value>().unwrap())
}

fn shutdown(app: &App<MockRuntime>) {
    tauri::async_runtime::block_on(app.gitprompt().unwrap().shutdown());
}

#[test]
fn setup_starts_the_daemon_and_status_reports_it() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let status = invoke(&webview, "status", json!({})).unwrap();
    assert_eq!(status["daemon"], json!({"state": "running"}));
    assert_eq!(status["data_dir"], json!(h.home().display().to_string()));
    assert_eq!(status["version"], json!(env!("CARGO_PKG_VERSION")));
    let port = status["port"].as_u64().unwrap();
    assert_ne!(port, 0, "port 0 reports the bound port");
    assert_eq!(status["url"], json!(format!("http://127.0.0.1:{port}/")));
    // The daemon really listens there.
    std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
    shutdown(&app);
}

#[test]
fn every_listed_command_is_registered() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let unknown = invoke(&webview, "nope", json!({})).unwrap_err().to_string();
    for command in COMMANDS {
        if let Err(error) = invoke(&webview, command, json!({"dry_run": true})) {
            assert_ne!(error.to_string(), unknown.replace("nope", command), "{command} is not registered");
            assert!(!error.to_string().contains("not found") || error.is_object(), "{command}: {error}");
        }
    }
    shutdown(&app);
}

#[test]
fn bus_notices_are_emitted_as_captured_events() {
    let h = Harness::new();
    let app = h.app();
    let (tx, rx) = mpsc::channel();
    app.listen_any(CAPTURED_EVENT, move |event| {
        let _ = tx.send(event.payload().to_owned());
    });
    let service = app.gitprompt().unwrap().service().unwrap();
    service.bus().publish(Notice { prompts: 1, events: 2 });
    let payload: Value = serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    assert_eq!(payload, json!({"prompts": 1, "events": 2}));
    shutdown(&app);
}

#[test]
fn errors_are_status_and_detail() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let error = invoke(&webview, "connect", json!({"source": "no-such-tool"})).unwrap_err();
    assert_eq!(error["status"], json!(404));
    assert!(error["detail"].as_str().unwrap().contains("no-such-tool"), "{error}");
    shutdown(&app);
}

#[test]
fn sources_can_be_disabled_and_enabled() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    assert_eq!(invoke(&webview, "disable", json!({"source": "codex"})).unwrap(), json!({"enabled": false}));
    // A disabled source can't be connected (409, as on the API).
    assert_eq!(invoke(&webview, "connect", json!({"source": "codex", "dry_run": true})).unwrap_err()["status"], json!(409));
    assert_eq!(invoke(&webview, "enable", json!({"source": "codex"})).unwrap(), json!({"enabled": true}));
    shutdown(&app);
}

#[test]
fn connect_and_disconnect_dry_runs_change_nothing() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let change = invoke(&webview, "connect", json!({"source": "claude-code", "dry_run": true})).unwrap();
    assert_eq!(change["applied"], json!(false));
    assert!(!change["diff"].as_str().unwrap().is_empty());
    assert!(!h.user_home().join(".claude/settings.json").exists());
    let change = invoke(&webview, "disconnect", json!({"source": "claude-code", "dry_run": true})).unwrap();
    assert_eq!(change["applied"], json!(false));
    shutdown(&app);
}

#[test]
fn connect_applies_by_default() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let change = invoke(&webview, "connect", json!({"source": "claude-code"})).unwrap();
    assert_eq!(change["applied"], json!(true));
    assert!(h.user_home().join(".claude/settings.json").is_file());
    shutdown(&app);
}

#[test]
fn rotate_token_and_compact_are_dry_runs_by_default() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let token = std::fs::read_to_string(h.settings().token_file()).unwrap();
    assert_eq!(invoke(&webview, "rotate_token", json!({})).unwrap()["applied"], json!(false));
    assert_eq!(std::fs::read_to_string(h.settings().token_file()).unwrap(), token);
    assert_eq!(invoke(&webview, "rotate_token", json!({"dry_run": false})).unwrap()["applied"], json!(true));
    assert_ne!(std::fs::read_to_string(h.settings().token_file()).unwrap(), token);

    assert_eq!(invoke(&webview, "compact", json!({})).unwrap()["applied"], json!(false));
    assert_eq!(invoke(&webview, "compact", json!({"older_than_days": 0})).unwrap_err()["status"], json!(400));
    assert_eq!(invoke(&webview, "compact", json!({"dry_run": false, "older_than_days": 30})).unwrap()["applied"], json!(true));
    shutdown(&app);
}

#[test]
fn export_to_file_writes_the_rendered_export() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let target = h.root.path().join("overview.json");
    let saved = invoke(&webview, "export_to_file", json!({"kind": "overview", "path": target, "query": {"format": "json"}})).unwrap();
    let written = std::fs::read(&target).unwrap();
    assert_eq!(saved["path"], json!(target.display().to_string()));
    assert_eq!(saved["bytes"], json!(written.len()));
    assert_eq!(saved["media_type"].as_str().unwrap().split(';').next(), Some("application/json"));
    assert!(saved["filename"].as_str().unwrap().starts_with("gitprompt-overview"), "{saved}");
    serde_json::from_slice::<Value>(&written).unwrap();
    shutdown(&app);
}

#[test]
fn export_to_file_refuses_relative_paths_and_missing_folders() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    let relative = invoke(&webview, "export_to_file", json!({"kind": "overview", "path": "out.html"})).unwrap_err();
    assert_eq!(relative["status"], json!(400));
    let missing = h.root.path().join("missing/out.html");
    let error = invoke(&webview, "export_to_file", json!({"kind": "overview", "path": missing})).unwrap_err();
    assert_eq!(error["status"], json!(404));
    // A session export without an id is refused by the service (422), nothing written.
    let target = h.root.path().join("session.html");
    let error = invoke(&webview, "export_to_file", json!({"kind": "session", "path": target})).unwrap_err();
    assert_eq!(error["status"], json!(422));
    assert!(!target.exists());
    shutdown(&app);
}

#[test]
fn open_data_folder_and_reveal_backup_use_the_opener() {
    let h = Harness::new();
    let app = h.app();
    let webview = webview(&app);
    assert_eq!(invoke(&webview, "open_data_folder", json!({})).unwrap(), json!(h.home().display().to_string()));
    assert_eq!(*h.recorder.opened.lock().unwrap(), [h.home()]);

    assert_eq!(invoke(&webview, "reveal_backup", json!({"name": "settings.json.bak"})).unwrap_err()["status"], json!(404));
    assert_eq!(invoke(&webview, "reveal_backup", json!({"name": "../token"})).unwrap_err()["status"], json!(400));
    let backups = h.settings().backups();
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::write(backups.join("settings.json.bak"), b"{}").unwrap();
    let path = invoke(&webview, "reveal_backup", json!({"name": "settings.json.bak"})).unwrap();
    assert_eq!(path, json!(backups.join("settings.json.bak").display().to_string()));
    assert_eq!(*h.recorder.revealed.lock().unwrap(), [backups.join("settings.json.bak")]);
    shutdown(&app);
}

#[test]
fn a_daemon_already_running_is_external_and_store_commands_answer_409() {
    let h = Harness::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let options = DaemonOptions { addr: Some("127.0.0.1:0".parse().unwrap()), ..DaemonOptions::default() };
    let cli = runtime.block_on(Daemon::start(h.settings(), Harness::options(), options)).unwrap();
    let h = h.port(cli.local_addr().port());
    let app = h.app();
    let webview = webview(&app);
    let status = invoke(&webview, "status", json!({})).unwrap();
    assert_eq!(status["daemon"], json!({"state": "external"}));
    assert_eq!(status["url"], json!(format!("http://{}/", cli.local_addr())));
    let error = invoke(&webview, "disable", json!({"source": "codex"})).unwrap_err();
    assert_eq!(error["status"], json!(409));
    // Folder commands don't need the store.
    invoke(&webview, "open_data_folder", json!({})).unwrap();
    shutdown(&app);
    runtime.block_on(cli.shutdown()).unwrap();
}

#[test]
fn a_port_in_use_is_a_failed_start_not_a_crash() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let h = Harness::new().port(taken.local_addr().unwrap().port());
    let app = h.app();
    let webview = webview(&app);
    let status = invoke(&webview, "status", json!({})).unwrap();
    assert_eq!(status["daemon"]["state"], json!("failed"));
    assert!(status["daemon"]["detail"].as_str().unwrap().contains("cannot listen"), "{status}");
    assert_eq!(invoke(&webview, "compact", json!({})).unwrap_err()["status"], json!(503));
}

#[test]
fn shutdown_stops_the_daemon_and_releases_the_lock() {
    let h = Harness::new();
    let app = h.app();
    let state = app.gitprompt().unwrap();
    let port = state.addr().port();
    shutdown(&app);
    assert_eq!(state.status(), DaemonStatus::Stopped);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "still listening");
    assert_eq!(state.service().err().unwrap().status, 503);
    // Another daemon can take the data directory now.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let options = DaemonOptions { addr: Some("127.0.0.1:0".parse().unwrap()), ..DaemonOptions::default() };
    let next = runtime.block_on(Daemon::start(h.settings(), Harness::options(), options)).unwrap();
    runtime.block_on(next.shutdown()).unwrap();
}
