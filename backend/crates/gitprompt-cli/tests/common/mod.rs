//! CLI test harness: a temp data directory and home, the `gitprompt` binary run with exactly that
//! environment (never the developer's: `env_clear`), and in-process helpers to put data in place
//! through the server's Service or to run a Daemon on loopback.
#![allow(dead_code)]

pub mod binary;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

use gitprompt_core::Settings;
use gitprompt_server::{App, AppOptions, Daemon, DaemonOptions, FixedVersion, NoGit};

pub struct Home {
    pub settings: Settings,
    _tmp: TempDir,
}

/// `<tmp>/gitprompt` as GITPROMPT_HOME and `<tmp>/home` as HOME.
pub fn home() -> Home {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().canonicalize().expect("canonical");
    let settings = Settings::new(root.join("gitprompt"), root.join("home"));
    std::fs::create_dir_all(&settings.user_home).expect("mkdir home");
    Home { settings, _tmp: tmp }
}

impl Home {
    /// The binary with this home only (`env_clear`), port 7878 unless [`Self::cmd_on`].
    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut command = Command::cargo_bin("gitprompt").expect("binary");
        command
            .env_clear()
            .env("HOME", &self.settings.user_home)
            .env("GITPROMPT_HOME", &self.settings.home)
            .args(args)
            .timeout(std::time::Duration::from_secs(20));
        command
    }

    /// The binary talking to the daemon (or nothing) on `port`.
    pub fn cmd_on(&self, port: u16, args: &[&str]) -> Command {
        let mut command = self.cmd(args);
        command.env("GITPROMPT_PORT", port.to_string());
        command
    }

    pub fn options(&self) -> AppOptions {
        AppOptions { tool_version: Arc::new(FixedVersion(None)), git: Arc::new(NoGit), ..AppOptions::default() }
    }

    /// Ingest payloads through the server's Service (in-process), then close the app.
    pub async fn ingest(&self, payloads: &[(&str, Value)]) {
        let app = App::new(self.settings.clone(), self.options()).expect("app");
        let service = app.service();
        for (source, payload) in payloads {
            service.ingest(source, payload.clone()).await.expect("ingest");
        }
    }

    /// A daemon for this home on a loopback port of its own.
    pub async fn daemon(&self) -> Daemon {
        let options = DaemonOptions { addr: Some("127.0.0.1:0".parse().expect("addr")), ..DaemonOptions::default() };
        Daemon::start(self.settings.clone(), self.options(), options).await.expect("daemon starts")
    }

    pub fn db_path(&self) -> PathBuf {
        self.settings.db_path()
    }
}

/// A loopback port nothing listens on (bound, then released).
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port()
}

pub fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Typed texts of the prompts a running daemon stored, newest first.
pub async fn prompt_texts(daemon: &Daemon) -> Vec<String> {
    let prompts = daemon.service().prompts(Default::default()).await.expect("prompts");
    prompts.into_iter().map(|p| p.prompt.unwrap_or_default()).collect()
}
