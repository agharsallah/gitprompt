//! The app under test: built on a data directory, filled with synthetic turns, and the dashboard
//! reads to time on it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use serde_json::Value;
use tempfile::TempDir;

use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_core::{JsonMap, Settings};
use gitprompt_server::{App, AppOptions, FixedVersion, GitProvider, NoGit, Service};

use crate::args::Args;
use crate::measure::get;
use crate::payloads::{Rng, turn_payloads};

/// Git with no repository whose context takes `delay_ms` to answer: 0 while the store is filled
/// and read alone, `--git-ms` under load (a cold cache or a big repository).
#[derive(Default)]
pub(crate) struct SlowGit {
    delay_ms: AtomicU64,
}

impl SlowGit {
    pub(crate) fn set_delay(&self, delay: Duration) {
        self.delay_ms.store(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}

impl GitProvider for SlowGit {
    fn context(&self, cwd: Option<&Path>) -> JsonMap {
        std::thread::sleep(Duration::from_millis(self.delay_ms.load(Ordering::Relaxed)));
        NoGit.context(cwd)
    }
    fn commits(&self, root: &Path, start: Option<&str>, end: &str, since: &str) -> CommitsAfter {
        NoGit.commits(root, start, end, since)
    }
    fn history(&self, root: &Path, since: &str, until: &str) -> Vec<Commit> {
        NoGit.history(root, since, until)
    }
}

/// How filling the store went.
pub(crate) struct Ingest {
    pub(crate) events: usize,
    pub(crate) bytes: usize,
    pub(crate) elapsed: Duration,
    pub(crate) index: Duration,
}

/// One dashboard read; `under_load`: also timed while ingest runs.
pub(crate) struct ReadSpec {
    pub(crate) name: &'static str,
    pub(crate) path: String,
    pub(crate) under_load: bool,
}

pub(crate) struct Bench {
    pub(crate) service: Service,
    pub(crate) router: Router,
    pub(crate) host: String,
    pub(crate) git: Arc<SlowGit>,
    pub(crate) home: PathBuf,
    _tmp: Option<TempDir>,
}

impl Bench {
    /// An app on `--home` or a fresh temp directory, with pinned tool versions and [`SlowGit`].
    pub(crate) fn open(args: &Args) -> Result<Bench, String> {
        let (home, tmp) = if let Some(dir) = &args.home {
            (dir.clone(), None)
        } else {
            let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
            (tmp.path().join("gitprompt"), Some(tmp))
        };
        let user_home = home.parent().map_or_else(|| home.join("home"), |p| p.join("bench-home"));
        std::fs::create_dir_all(&user_home).map_err(|e| format!("{}: {e}", user_home.display()))?;
        let settings = Settings::new(&home, &user_home);
        let host = format!("{}:{}", settings.host(), settings.port);
        let git = Arc::new(SlowGit::default());
        let options = AppOptions { tool_version: Arc::new(FixedVersion(Some("2.1.0".into()))), git: git.clone(), ..AppOptions::default() };
        let app = App::new(settings, options).map_err(|e| e.to_string())?;
        Ok(Bench { service: app.service(), router: app.router(), host, git, home, _tmp: tmp })
    }

    /// Ingest `--prompts` turns one payload at a time, as hooks deliver them, then index.
    pub(crate) async fn fill(&self, args: &Args) -> Result<Ingest, String> {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let (mut events, mut bytes, mut elapsed) = (0, 0, Duration::ZERO);
        for i in 0..args.prompts {
            for payload in turn_payloads(&mut rng, i, args.events_per_prompt) {
                bytes += serde_json::to_vec(&payload).map_err(|e| e.to_string())?.len(); // what a hook sends
                let started = Instant::now();
                self.service.ingest("claude-code", payload).await.map_err(|e| format!("ingest: {e}"))?;
                elapsed += started.elapsed();
                events += 1;
            }
        }
        let started = Instant::now();
        self.service.ensure_index().await.map_err(|e| format!("index: {e}"))?;
        Ok(Ingest { events, bytes, elapsed, index: started.elapsed() })
    }

    /// The main dashboard reads, on a prompt and a session from the middle of the store.
    pub(crate) async fn reads(&self) -> Result<Vec<ReadSpec>, String> {
        let (_, list) = get(&self.router, &self.host, "/api/prompts?limit=50").await?;
        let list: Value = serde_json::from_slice(&list).map_err(|e| format!("/api/prompts: {e}"))?;
        let sample = list.as_array().and_then(|l| l.get(l.len() / 2)).ok_or("no prompt listed")?;
        let id = sample["id"].as_i64().ok_or("a prompt without id")?;
        let session = sample["fields"]["session_id"].as_str().ok_or("a prompt without session")?;
        let read = |name, path: String, under_load| ReadSpec { name, path, under_load };
        Ok(vec![
            read("prompts (limit=50)", "/api/prompts?limit=50".into(), true),
            read("prompt detail", format!("/api/prompts/{id}"), true),
            read("stats by day", "/api/stats?group_by=day".into(), true),
            read("stats by model", "/api/stats?group_by=model".into(), false),
            read("stats by tool", "/api/stats?group_by=tool".into(), false),
            read("sources", "/api/sources".into(), true),
            read("data", "/api/data".into(), false),
            read("session story", format!("/api/stories/session/{session}"), false),
            read("search", "/api/search?q=retry".into(), false),
            read("fields", "/api/fields".into(), false),
        ])
    }

    pub(crate) fn db_bytes(&self) -> u64 {
        std::fs::metadata(self.home.join("gitprompt.db")).map(|m| m.len()).unwrap_or_default()
    }
}
