//! Runtime settings: where GitPrompt keeps its data and how the receiver is reached.
//!
//! | variable              | meaning                                         | default        |
//! |-----------------------|-------------------------------------------------|----------------|
//! | `GITPROMPT_HOME`      | data directory (db, token, backups, user specs) | `~/.gitprompt` |
//! | `GITPROMPT_PORT`      | port the receiver and dashboard listen on       | `7878`         |
//! | `GITPROMPT_LOG_LEVEL` | log level or filter (`gitprompt_server::logging`) | `info` for GitPrompt |
//!
//! The host is fixed to loopback on purpose: the prompt API has no auth beyond the Host check, so
//! binding elsewhere would expose every stored prompt to the network.

use std::path::{Path, PathBuf};

use crate::files::{atomic_write, create_exclusive};
use crate::{Env, Error, Result, random};

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
    /// `GITPROMPT_LOG_LEVEL`. `HOME` (or `USERPROFILE`) must be set: it is the base for `~`.
    pub fn from_env(env: Env) -> Result<Self> {
        let user_home =
            PathBuf::from(env.get("HOME").or_else(|| env.get("USERPROFILE")).ok_or_else(|| Error::Invalid("HOME is not set".into()))?);
        let home = match env.get("GITPROMPT_HOME") {
            Some(dir) => expand_tilde(dir, &user_home),
            None => user_home.join(".gitprompt"),
        };
        let port = match env.get("GITPROMPT_PORT") {
            Some(text) => parse_port(text)?,
            None => DEFAULT_PORT,
        };
        let log_level = env.get("GITPROMPT_LOG_LEVEL").unwrap_or("warning").to_owned();
        Ok(Settings { home, user_home, port, log_level, env })
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
        format!("http://{HOST}:{}", self.port)
    }

    pub fn db_path(&self) -> PathBuf {
        self.home.join("gitprompt.db")
    }

    /// Drop a YAML spec here to add or fix a source without a new release.
    pub fn user_sources(&self) -> PathBuf {
        self.home.join("sources")
    }

    pub fn backups(&self) -> PathBuf {
        self.home.join("backups")
    }

    pub fn token_file(&self) -> PathBuf {
        self.home.join("token")
    }

    /// Where the hook forwarder queues payloads while the daemon is down (new in the Rust port).
    pub fn spool_dir(&self) -> PathBuf {
        self.home.join("spool")
    }

    /// Held by the running daemon so a second one (CLI or desktop app) refuses to start (new).
    pub fn lock_file(&self) -> PathBuf {
        self.home.join("daemon.lock")
    }

    /// Shared secret that hook calls must present. Created once (URL-safe, ≥ 32 random bytes), in a
    /// home directory of mode 0700 and a file of mode 0600; later calls return the same value.
    /// An empty token file is replaced by a new token.
    pub fn token(&self) -> Result<String> {
        let path = self.token_file();
        if let Some(token) = read_token(&path)? {
            return Ok(token);
        }
        self.create_home()?;
        let token = new_token()?;
        if create_exclusive(&path, &token, TOKEN_MODE)? {
            return Ok(token);
        }
        // Another process created it first (or left an empty file): use or replace what is there.
        match read_token(&path)? {
            Some(theirs) => Ok(theirs),
            None => self.rotate_token(),
        }
    }

    /// Replace the token with a new random one (Settings › Security). Returns the new token.
    pub fn rotate_token(&self) -> Result<String> {
        self.create_home()?;
        let token = new_token()?;
        atomic_write(&self.token_file(), &token, TOKEN_MODE)?;
        Ok(token)
    }

    /// The data directory, created owner-only if missing (an existing one keeps its mode).
    fn create_home(&self) -> Result<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&self.home)?;
        Ok(())
    }
}

const TOKEN_MODE: u32 = 0o600;

fn new_token() -> Result<String> {
    Ok(random::token_urlsafe::<32>()?)
}

/// The trimmed token in `path`; `None` when the file is missing or blank.
fn read_token(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_owned()).filter(|t| !t.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// `~` or `~/rest` resolved against `user_home`; anything else as given.
pub(crate) fn expand_tilde(path: &str, user_home: &Path) -> PathBuf {
    match path.strip_prefix('~') {
        Some("") => user_home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => user_home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(path),
    }
}

fn parse_port(text: &str) -> Result<u16> {
    text.trim().parse().map_err(|_| Error::Invalid(format!("GITPROMPT_PORT must be a port number (0-65535), got {text:?}")))
}
