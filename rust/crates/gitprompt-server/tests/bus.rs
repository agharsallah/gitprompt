//! The live-update bus: fan-out to every subscriber, never blocking the publisher. Port of
//! test_bus_drops_notices_for_a_stalled_subscriber (test_extension_points.py), adapted to
//! broadcast semantics: a stalled subscriber keeps at most `backlog` notices.

use std::time::Duration;

use gitprompt_server::{Bus, Notice};
use pretty_assertions::assert_eq;

fn notice(prompts: u64) -> Notice {
    Notice { prompts, events: 0 }
}

#[tokio::test]
async fn every_subscriber_receives_each_notice() {
    let bus = Bus::new(10);
    let (mut a, mut b) = (bus.subscribe(), bus.subscribe());
    bus.publish(notice(1));
    let within = Duration::from_secs(1);
    assert_eq!(tokio::time::timeout(within, a.next()).await.expect("a"), Some(notice(1)));
    assert_eq!(tokio::time::timeout(within, b.next()).await.expect("b"), Some(notice(1)));
}

#[tokio::test]
async fn a_subscriber_only_sees_notices_published_after_it_subscribed() {
    let bus = Bus::new(10);
    bus.publish(notice(1));
    let mut late = bus.subscribe();
    bus.publish(notice(2));
    assert_eq!(late.try_next(), Some(notice(2)));
    assert_eq!(late.try_next(), None);
}

#[tokio::test]
async fn a_stalled_subscriber_keeps_at_most_the_backlog_and_never_errors() {
    let bus = Bus::new(1);
    let mut stalled = bus.subscribe();
    bus.publish(notice(1));
    bus.publish(notice(2)); // over the backlog: an older notice is dropped, nothing is raised
    assert!(stalled.try_next().is_some());
    assert_eq!(stalled.try_next(), None);
    bus.publish(notice(3));
    assert_eq!(stalled.try_next(), Some(notice(3))); // and it keeps receiving
}

#[tokio::test]
async fn publishing_without_subscribers_does_nothing() {
    let bus = Bus::new(1);
    bus.publish(notice(1));
    let gone = bus.subscribe();
    drop(gone);
    bus.publish(notice(2));
    assert_eq!(bus.subscribers(), 0);
}

#[tokio::test]
async fn a_notice_serializes_as_the_streams_payload() {
    assert_eq!(serde_json::to_value(Notice { prompts: 1, events: 2 }).expect("json"), serde_json::json!({"prompts": 1, "events": 2}));
}
