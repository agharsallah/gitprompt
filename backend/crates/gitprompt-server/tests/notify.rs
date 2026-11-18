//! The notifier in a running daemon: a capture in, the notification decided on the bus, the rate
//! limit holding the rest back. Time from a `ManualClock`; waits on the bus, never on a duration.
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

use common::harness;
use gitprompt_insights::notify::LIMIT;
use gitprompt_server::model::{Notification, NotificationKind, Target};
use gitprompt_server::{Daemon, DaemonOptions, ManualClock, Service, Subscription};
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::json;

const MIN: SignedDuration = SignedDuration::from_mins(1);

/// Synthetic, built at run time so no file holds a key-shaped string.
fn key() -> String {
    "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd"
}

fn on_any_port() -> DaemonOptions {
    DaemonOptions { addr: Some("127.0.0.1:0".parse().expect("addr")), ..DaemonOptions::default() }
}

async fn prompt(service: &Service, n: usize, text: &str) {
    let payload =
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": format!("p{n}"), "session_id": format!("s{n}"), "prompt": text});
    service.ingest("claude-code", payload).await.expect("prompt");
}

async fn stop(service: &Service, n: usize) {
    let payload = json!({"hook_event_name": "Stop", "prompt_id": format!("p{n}"), "session_id": format!("s{n}")});
    service.ingest("claude-code", payload).await.expect("stop");
}

/// A turn of `minutes` on `clock`: its prompt, then its end.
async fn turn(service: &Service, clock: &ManualClock, n: usize, minutes: i32) {
    prompt(service, n, &format!("refactor module {n}")).await;
    clock.advance(minutes * MIN);
    stop(service, n).await;
}

/// The id of the newest prompt.
async fn newest(service: &Service) -> i64 {
    service.prompts(gitprompt_server::model::PromptQuery::default()).await.expect("prompts")[0].id
}

async fn next(notes: &mut Subscription<Notification>) -> Notification {
    notes.next().await.expect("the bus is open")
}

/// A daemon on a moving clock, its spool drained (the notifier listens from then on).
async fn started(clock: &ManualClock, env: &[(&str, &str)]) -> (Daemon, common::Harness) {
    let h = env.iter().fold(harness().moving_clock(clock), |h, (name, value)| h.env(name, value));
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    daemon.spool_drained().await;
    (daemon, h)
}

#[tokio::test]
async fn a_long_turn_notifies_once_and_the_limit_holds_the_next_ones_back() {
    let clock = ManualClock::new("2026-11-18T08:00:00Z".parse::<Timestamp>().unwrap());
    let (daemon, _h) = started(&clock, &[]).await;
    let service = daemon.service();
    let mut notes = service.bus().notifications();

    turn(&service, &clock, 0, 12).await;
    let first = next(&mut notes).await;
    let expected = Notification {
        kind: NotificationKind::LongTurn,
        title: "A long turn finished".into(),
        body: "refactor module 0 ran for 12 min".into(),
        target: Target::Prompt { id: newest(&service).await },
    };
    assert_eq!(first, expected);

    turn(&service, &clock, 1, 3).await; // short: nothing
    for n in 2..=LIMIT.max + 1 {
        turn(&service, &clock, n, 11).await;
    }
    // The limit's worth of long turns notified; the one after is held back, so the next
    // notification on the bus is the leak captured after it.
    prompt(&service, 99, &format!("use {} please", key())).await;
    let bodies: Vec<String> = [next(&mut notes).await, next(&mut notes).await, next(&mut notes).await].map(|n| n.body).into();
    assert_eq!(bodies[..2], ["refactor module 2 ran for 11 min", "refactor module 3 ran for 11 min"]);
    assert_eq!(bodies[2], "OpenAI API key sk-…y6Hd was captured", "the fourth long turn in the hour was held back");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_kind_switched_off_is_never_notified() {
    let clock = ManualClock::new("2026-11-18T08:00:00Z".parse::<Timestamp>().unwrap());
    let (daemon, _h) = started(&clock, &[("GITPROMPT_NOTIFY_LONG_TURN", "off")]).await;
    let service = daemon.service();
    let mut notes = service.bus().notifications();
    turn(&service, &clock, 0, 30).await;
    prompt(&service, 1, &format!("use {} please", key())).await;
    assert_eq!(next(&mut notes).await.kind, NotificationKind::PossibleLeak, "the long turn before it made none");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn what_was_captured_before_the_daemon_started_is_never_notified() {
    let clock = ManualClock::new("2026-11-18T08:00:00Z".parse::<Timestamp>().unwrap());
    let h = harness().moving_clock(&clock);
    let api = gitprompt_server::App::new(h.settings.clone(), h.options.clone()).expect("app");
    let before = api.service();
    prompt(&before, 0, &format!("use {} please", key())).await;
    prompt(&before, 1, "a turn still running").await;
    drop((before, api));

    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), on_any_port()).await.expect("start");
    daemon.spool_drained().await;
    let service = daemon.service();
    let mut notes = service.bus().notifications();
    clock.advance(20 * MIN);
    stop(&service, 1).await; // started before: not watched
    turn(&service, &clock, 2, 15).await;
    let note = next(&mut notes).await;
    assert_eq!(note.body, "refactor module 2 ran for 15 min", "neither the old finding nor the old turn");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn telemetry_gone_quiet_while_the_hooks_of_its_tool_keep_sending_is_notified() {
    let clock = ManualClock::new("2026-11-18T08:00:00Z".parse::<Timestamp>().unwrap());
    let (daemon, _h) = started(&clock, &[("GITPROMPT_NOTIFY_QUIET_CONNECTOR_MINUTES", "20")]).await;
    let service = daemon.service();
    let mut notes = service.bus().notifications();
    let telemetry = common::otlp(json!({}), json!({"event.name": "api_request", "prompt.id": "p0", "session.id": "s0"}));
    service.ingest_otlp("claude-code-otel", gitprompt_server::service::Signal::Logs, telemetry).await.expect("telemetry");
    clock.advance(25 * MIN);
    prompt(&service, 1, "the hooks still send").await;
    let note = next(&mut notes).await;
    assert_eq!(note.title, "Claude Code telemetry (model, tokens, cost) has gone quiet");
    assert_eq!(note.target, Target::Source { id: "claude-code-otel".into() });
    daemon.shutdown().await.expect("shutdown");
}
