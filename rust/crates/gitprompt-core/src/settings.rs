//! Runtime settings: where GitPrompt keeps its data and how the receiver is reached.
//!
//! | variable              | meaning                                         | default        |
//! |-----------------------|-------------------------------------------------|----------------|
//! | `GITPROMPT_HOME`      | data directory (db, token, backups, user specs) | `~/.gitprompt` |
//! | `GITPROMPT_PORT`      | port the receiver and dashboard listen on       | `7878`         |
//! | `GITPROMPT_LOG_LEVEL` | log filter                                      | `warning`      |
//!
//! The host is fixed to loopback on purpose: the prompt API has no auth beyond the Host check, so
//! binding elsewhere would expose every stored prompt to the network.

use std::path::PathBuf;

use crate::{Env, Result};

pub const HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 7878;

#[derive(Clone, Debug)]
pub struct Settings {
    /// Data directory.
    pub home: PathBuf,
    /// Base for `~` in source specs (a temp dir in tests, never the real home).
    pub user_home: PathBuf,
    pub port: u16,
    pub log_level: String,
    /// The environment specs and preferences read (`${CLAUDE_CONFIG_DIR:-...}`, `GITPROMPT_*`).
    pub env: Env,
}

impl Settings {
    /// Settings from an environment: `GITPROMPT_HOME` (with `~` expanded against `HOME`),
    /// `GITPROMPT_PORT` (an `Error::Invalid` naming the variable when it isn't a port number),
    /// `GITPROMPT_LOG_LEVEL`.
    pub fn from_env(_env: Env) -> Result<Self> {
        todo!()
    }

    /// Settings rooted in explicit directories, default port and log level, empty environment (tests).
    pub fn new(home: impl Into<PathBuf>, user_home: impl Into<PathBuf>) -> Self {
        Settings { home: home.into(), user_home: user_home.into(), port: DEFAULT_PORT, log_level: "warning".into(), env: Env::default() }
    }

    pub fn host(&self) -> &'static str {
        HOST
    }

    /// `http://127.0.0.1:<port>`: what connected hooks call.
    pub fn receiver(&self) -> String {
        todo!()
    }

    pub fn db_path(&self) -> PathBuf {
        todo!()
    }

    /// Drop a YAML spec here to add or fix a source without a new release.
    pub fn user_sources(&self) -> PathBuf {
        todo!()
    }

    pub fn backups(&self) -> PathBuf {
        todo!()
    }

    pub fn token_file(&self) -> PathBuf {
        todo!()
    }

    /// Where the hook forwarder queues payloads while the daemon is down (new in the Rust port).
    pub fn spool_dir(&self) -> PathBuf {
        todo!()
    }

    /// Held by the running daemon so a second one (CLI or desktop app) refuses to start (new).
    pub fn lock_file(&self) -> PathBuf {
        todo!()
    }

    /// Shared secret that hook calls must present. Created once (URL-safe, ≥ 32 random bytes), in a
    /// home directory of mode 0700 and a file of mode 0600; later calls return the same value.
    pub fn token(&self) -> Result<String> {
        todo!()
    }

    /// Replace the token with a new random one (Settings › Security). Returns the new token.
    pub fn rotate_token(&self) -> Result<String> {
        todo!()
    }
}
