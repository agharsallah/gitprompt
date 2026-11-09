//! The Daemon: a running GitPrompt on loopback (port 0), one per data directory, with graceful
//! shutdown and background retention. Loopback sockets only. (Draining the hook spool at start is
//! covered with the forwarder in the CLI tests, which can produce a spool.)

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use common::harness;
use gitprompt_server::{Daemon, DaemonError, DaemonOptions};
use gitprompt_store::Store;
use pretty_assertions::assert_eq;
use serde_json::json;

fn on_any_port() -> DaemonOptions {
    DaemonOptions { addr: Some("127.0.0.1:0".parse().expect("addr")), ..DaemonOptions::default() }
}

/// GET over a real socket (std, off the runtime); the raw HTTP/1.1 answer.
async fn http_get(addr: SocketAddr, path: &str) -> std::io::Result<String> {
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", addr.port());
    tokio::task::spawn_blocking(move || {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.write_all(request.as_bytes())?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer)?;
        Ok(answer)
    })
    .await
    .expect("blocking task")
}

#[tokio::test]
async fn a_started_daemon_serves_on_loopback_at_its_bound_port() {
    let h = harness();
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    let addr = daemon.local_addr();
    assert!(addr.ip().is_loopback() && addr.port() != 0, "{addr}");
    let answer = http_get(addr, "/healthz").await.expect("connect");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.ends_with(r#"{"ok":true}"#), "{answer}");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_second_daemon_for_the_same_data_directory_refuses_to_start() {
    let h = harness();
    let first = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("first");
    let second = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await;
    let error = second.err().expect("the second start fails");
    assert!(matches!(&error, DaemonError::AlreadyRunning { lock_file } if *lock_file == h.settings.lock_file()), "{error:?}");
    assert!(error.to_string().contains("already running"), "{error}");
    first.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn daemons_of_different_data_directories_run_side_by_side() {
    let (a, b) = (harness(), harness());
    let first = Daemon::start(a.settings.clone(), a.options.clone(), on_any_port()).await.expect("first");
    let second = Daemon::start(b.settings.clone(), b.options.clone(), on_any_port()).await.expect("second");
    first.shutdown().await.expect("shutdown");
    second.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn shutdown_stops_serving_and_releases_the_lock() {
    let h = harness();
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    let addr = daemon.local_addr();
    daemon.shutdown().await.expect("shutdown");
    assert!(http_get(addr, "/healthz").await.is_err(), "still serving after shutdown");
    let again = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("starts again");
    again.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_lock_file_left_by_a_crashed_daemon_does_not_block_a_start() {
    let h = harness();
    std::fs::create_dir_all(&h.settings.home).expect("mkdir");
    std::fs::write(h.settings.lock_file(), "12345").expect("stale lock");
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_port_in_use_fails_the_start_and_leaves_no_lock_behind() {
    let h = harness();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = taken.local_addr().expect("addr");
    let busy = DaemonOptions { addr: Some(addr), ..DaemonOptions::default() };
    let error = Daemon::start(h.settings.clone(), h.options.clone(), busy).await.err().expect("bind fails");
    assert!(matches!(error, DaemonError::Bind { addr: a, .. } if a == addr), "{error:?}");
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("lock was released");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn the_daemon_serves_the_same_service_its_routes_use() {
    let h = harness();
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    daemon
        .service()
        .ingest("claude-code", json!({"hook_event_name": "UserPromptSubmit", "prompt_id": "p1", "session_id": "s", "prompt": "hi"}))
        .await
        .expect("ingest");
    let answer = http_get(daemon.local_addr(), "/api/prompts").await.expect("connect");
    assert!(answer.starts_with("HTTP/1.1 200") && answer.contains(r#""prompt":"hi""#), "{answer}");
    daemon.shutdown().await.expect("shutdown");
}

/// Retention set to 30 days, and `n` stored events.
fn retention_of_30_days_and_events(h: &common::Harness, n: usize) {
    std::fs::create_dir_all(&h.settings.home).expect("mkdir");
    std::fs::write(h.settings.home.join("preferences.json"), r#"{"event_retention_days": 30}"#).expect("preferences");
    let store = Store::open(&h.settings.db_path()).expect("store");
    let fields = json!({"event": "Stop", "session_id": "s1"}).as_object().cloned().expect("object");
    for _ in 0..n {
        store.add_event("claude-code", &fields, &json!({})).expect("event");
    }
}

#[tokio::test]
async fn the_first_retention_pass_runs_before_start_returns() {
    let h = harness().clock(Timestamp::now() + SignedDuration::from_hours(24 * 40)); // every stored event is 40 days old
    retention_of_30_days_and_events(&h, 2);
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    assert_eq!(daemon.service().data().await.expect("data").events, vec![]);
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn retention_repeats_on_its_interval() {
    let h = harness().clock(Timestamp::now() + SignedDuration::from_hours(24 * 40));
    retention_of_30_days_and_events(&h, 0);
    let options = DaemonOptions { retention_every: Duration::from_millis(20), ..on_any_port() };
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), options).await.expect("start");
    let service = daemon.service();
    service.ingest("claude-code", json!({"hook_event_name": "Stop", "session_id": "s1"})).await.expect("ingest");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut remaining = service.data().await.expect("data").events.len();
    while remaining > 0 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await; // polling a condition, bounded by the deadline
        remaining = service.data().await.expect("data").events.len();
    }
    assert_eq!(remaining, 0, "a later pass removed the expired event");
    daemon.shutdown().await.expect("shutdown");
}

#[test]
fn by_default_the_daemon_listens_on_the_settings_port_and_runs_retention_every_6_hours() {
    let options = DaemonOptions::default();
    assert_eq!((options.addr, options.retention_every), (None, Duration::from_secs(6 * 3600)));
}
