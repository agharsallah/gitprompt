//! The weekly recap in a running daemon: one wait until its day and hour on a `ManualClock`, the
//! recap on the bus once, a recap missed while the daemon was down sent once at the next start and
//! never again after a restart. Waits on the bus, never on a duration.
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

use std::time::Duration;

use common::{Harness, harness};
use gitprompt_core::preferences::RangeId;
use gitprompt_server::model::{Notification, NotificationKind, Recap, Target};
use gitprompt_server::{Daemon, DaemonOptions, ManualClock, Service, Subscription};
use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::json;

/// A Wednesday.
const WEDNESDAY: &str = "2026-11-18T08:00:00Z";
/// The Monday after, at nine (the default day and hour, in the harness's UTC).
const DUE: &str = "2026-11-23T09:00:00Z";

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

/// Synthetic, built at run time so no file holds a key-shaped string.
fn key() -> String {
    "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd"
}

/// On any port, the recap reading the clock again every few milliseconds (the clock only moves
/// when the test moves it).
fn options() -> DaemonOptions {
    DaemonOptions { addr: Some("127.0.0.1:0".parse().unwrap()), recap_every: Duration::from_millis(5), ..DaemonOptions::default() }
}

async fn prompt(service: &Service, n: usize, text: &str) {
    let payload =
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": format!("p{n}"), "session_id": format!("s{n}"), "prompt": text});
    service.ingest("claude-code", payload).await.expect("prompt");
}

/// A week's work: two turns.
async fn work(service: &Service) {
    prompt(service, 0, "refactor the parser").await;
    prompt(service, 1, "write the tests").await;
}

/// A daemon on `h`, with a subscription to its notifications taken before its background work
/// runs (the test runtime runs one task at a time), returned once its start-up work is over.
async fn started(h: &Harness) -> (Daemon, Subscription<Notification>) {
    let daemon = Daemon::start(h.settings.clone(), h.options.clone(), options()).await.expect("start");
    let notes = daemon.service().bus().notifications();
    daemon.spool_drained().await;
    (daemon, notes)
}

async fn next(notes: &mut Subscription<Notification>) -> Notification {
    notes.next().await.expect("the bus is open")
}

/// The next notification after a new possible leak: the leak's, unless something came before it.
async fn next_after_a_leak(service: &Service, notes: &mut Subscription<Notification>) -> Notification {
    prompt(service, 9, &format!("use {} please", key())).await;
    next(notes).await
}

fn the_recap() -> Notification {
    Notification {
        kind: NotificationKind::WeeklyRecap,
        title: "Your week".into(),
        body: "This week: nothing redone · no commits.".into(),
        target: Target::Overview { range: Some(RangeId::Week) },
    }
}

#[tokio::test]
async fn the_recap_is_sent_once_when_its_instant_comes() {
    let clock = ManualClock::new(at(WEDNESDAY));
    let h = harness().moving_clock(&clock);
    let (daemon, mut notes) = started(&h).await;
    let service = daemon.service();
    work(&service).await;
    assert_eq!(service.recap().await.unwrap(), None, "nothing sent this week yet");

    clock.set(at(DUE));
    assert_eq!(next(&mut notes).await, the_recap());
    let sent = Recap { due: "2026-11-23T09:00:00.000000+00:00".into(), notification: the_recap() };
    assert_eq!(service.recap().await.unwrap(), Some(sent), "the card's recap for this week");
    assert_eq!(next_after_a_leak(&service, &mut notes).await.kind, NotificationKind::PossibleLeak, "no second recap");
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_recap_missed_while_down_is_sent_at_the_next_start_and_never_again() {
    let clock = ManualClock::new(at(WEDNESDAY));
    let h = harness().moving_clock(&clock);
    let app = gitprompt_server::App::new(h.settings.clone(), h.options.clone()).expect("app");
    work(&app.service()).await;
    drop(app);
    clock.set(at("2026-11-24T10:00:00Z")); // the Tuesday after: Monday's recap was missed

    let (daemon, mut notes) = started(&h).await;
    assert_eq!(next(&mut notes).await, the_recap());
    daemon.shutdown().await.expect("shutdown");

    let (again, mut notes) = started(&h).await;
    assert_eq!(next_after_a_leak(&again.service(), &mut notes).await.kind, NotificationKind::PossibleLeak, "not sent twice");
    again.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_recap_switched_off_is_never_sent() {
    let clock = ManualClock::new(at(WEDNESDAY));
    let h = harness().moving_clock(&clock).env("GITPROMPT_RECAP", "off");
    let (daemon, mut notes) = started(&h).await;
    let service = daemon.service();
    work(&service).await;
    clock.set(at(DUE));
    assert_eq!(service.due_recap().await.unwrap(), None);
    assert_eq!(next_after_a_leak(&service, &mut notes).await.kind, NotificationKind::PossibleLeak);
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn without_a_recap_this_week_the_route_answers_null() {
    let api = harness().clock(at(WEDNESDAY)).start();
    assert_eq!(api.json("/api/recap").await, serde_json::Value::Null);
}
