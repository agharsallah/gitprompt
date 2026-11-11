//! Dashboard reads never wait for a capture's git or `--version` probe. Those are child processes
//! (seconds on a cold cache or a big repository); a capture used to run them while holding the
//! database writer, and every prompt read refreshes the index through that writer, so each slow
//! probe stalled the dashboard ("slow request" warnings on /api/prompts, /api/stats).
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

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gitprompt_core::JsonMap;
use gitprompt_core::git::{Commit, CommitsAfter};
use gitprompt_server::GitProvider;

use common::{harness, hook_payload, with};
use serde_json::json;

/// Git whose context probe reports that it started, then blocks until released.
struct StuckGit {
    entered: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl GitProvider for StuckGit {
    fn context(&self, _cwd: Option<&Path>) -> JsonMap {
        self.entered.lock().unwrap().send(()).unwrap();
        let _ = self.release.lock().unwrap().recv_timeout(Duration::from_secs(10));
        JsonMap::new()
    }
    fn commits(&self, _root: &Path, _start: Option<&str>, end: &str, _since: &str) -> CommitsAfter {
        CommitsAfter { until: end.to_owned(), ..CommitsAfter::default() }
    }
    fn history(&self, _root: &Path, _since: &str, _until: &str) -> Vec<Commit> {
        Vec::new()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prompt_reads_answer_while_a_capture_waits_for_git() {
    // One prompt captured first, so the reads below have something to show and an index to check.
    let api = harness().start();
    api.ingest("claude-code", hook_payload(&api.root())).await;

    let (entered_tx, entered) = channel();
    let (release, release_rx) = channel();
    let mut stuck = harness();
    stuck.settings = api.settings.clone();
    stuck.options.git = Arc::new(StuckGit { entered: Mutex::new(entered_tx), release: Mutex::new(release_rx) });
    let stuck = stuck.start();

    let service = stuck.service();
    let payload = with(&hook_payload(&api.root()), json!({"prompt_id": "a-second-turn", "prompt": "another prompt"}));
    let capture = tokio::spawn(async move { service.ingest("claude-code", payload).await });
    tokio::task::spawn_blocking(move || entered.recv_timeout(Duration::from_secs(5)).expect("the capture asks git")).await.unwrap();

    // The capture is inside its git probe now.
    for path in ["/api/prompts", "/api/stats", "/api/sources"] {
        let response = tokio::time::timeout(Duration::from_secs(3), stuck.get(path).send())
            .await
            .unwrap_or_else(|_| panic!("{path} waited for the capture's git probe"));
        assert_eq!(response.status, 200, "{path}");
    }

    release.send(()).unwrap();
    capture.await.unwrap().expect("the capture completes once git answers");
    assert_eq!(stuck.json("/api/prompts").await.as_array().unwrap().len(), 2);
}
