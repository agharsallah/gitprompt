//! What deserves a notification: one table per rule, then the rate limit. Pure: "now" is passed in.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;

use gitprompt_core::preferences::Preferences;
use gitprompt_insights::notify::{ConnectorSeen, Event, Kind, LIMIT, LeakFound, Notification, State, TurnEnd, WAS_SENDING, decide};
use gitprompt_insights::target::Target;

const MIN: SignedDuration = SignedDuration::from_mins(1);

fn now() -> Timestamp {
    "2026-11-18T09:00:00Z".parse().unwrap()
}

fn ago(by: SignedDuration) -> Timestamp {
    now().checked_sub(by).unwrap()
}

fn turn(minutes: f64, failed: bool, text: Option<&str>) -> Event {
    Event::TurnEnded(TurnEnd { prompt: 7, failed, seconds: minutes * 60.0, text: text.map(Into::into), tool: "Claude Code".into() })
}

fn leak() -> Event {
    Event::LeakFound(LeakFound { kind: "AWS access key".into(), masked: "AKIA…7XQ".into() })
}

fn quiet(silent: SignedDuration, tool_silent: SignedDuration) -> Event {
    Event::ConnectorSeen(ConnectorSeen {
        id: "claude-code-otel".into(),
        name: "Claude Code telemetry".into(),
        last_at: ago(silent),
        tool_last_at: ago(tool_silent),
    })
}

/// The notification `event` makes on a fresh state.
fn once(event: &Event, prefs: &Preferences) -> Option<Notification> {
    decide(event, &mut State::default(), now(), prefs)
}

fn defaults() -> Preferences {
    Preferences::default()
}

/// A notification's title and body.
type Words = (&'static str, &'static str);

// --- long turn ------------------------------------------------------------------------------

#[test]
fn a_turn_notifies_once_it_ran_the_threshold_and_names_how_it_ended() {
    let prefs = Preferences { notify_long_turn_minutes: 10, ..defaults() };
    let cases: [(&str, Event, Option<Words>); 6] = [
        ("under the threshold", turn(9.9, false, Some("fix the parser")), None),
        ("at the threshold", turn(10.0, false, Some("fix the parser")), Some(("A long turn finished", "fix the parser ran for 10 min"))),
        ("failed", turn(75.0, true, Some("fix the parser")), Some(("A long turn failed", "fix the parser failed after 1 h 15 min"))),
        ("no typed text", turn(12.0, false, None), Some(("A long turn finished", "A Claude Code turn ran for 12 min"))),
        ("only a command", turn(12.0, false, Some("/review")), Some(("A long turn finished", "A Claude Code turn ran for 12 min"))),
        ("a broken duration", turn(f64::NAN, false, Some("x")), None),
    ];
    for (case, event, expected) in cases {
        let got = once(&event, &prefs);
        assert_eq!(got.as_ref().map(|n| (n.title.as_str(), n.body.as_str())), expected, "{case}");
        if let Some(n) = got {
            assert_eq!((n.kind, n.target), (Kind::LongTurn, Target::Prompt { id: 7 }), "{case}");
        }
    }
}

#[test]
fn the_long_turn_threshold_comes_from_the_preferences() {
    let event = turn(20.0, false, Some("x"));
    assert!(once(&event, &Preferences { notify_long_turn_minutes: 20, ..defaults() }).is_some());
    assert!(once(&event, &Preferences { notify_long_turn_minutes: 21, ..defaults() }).is_none());
}

// --- possible leak --------------------------------------------------------------------------

#[test]
fn a_new_possible_leak_notifies_masked_and_leads_to_the_leaks() {
    let expected = Notification {
        kind: Kind::PossibleLeak,
        title: "A possible leak".into(),
        body: "AWS access key AKIA…7XQ was captured".into(),
        target: Target::Leaks,
    };
    assert_eq!(once(&leak(), &defaults()), Some(expected));
}

// --- quiet connector ------------------------------------------------------------------------

#[test]
fn a_connector_is_quiet_when_silent_past_the_threshold_while_its_tool_sends() {
    let prefs = Preferences { notify_quiet_connector_minutes: 30, ..defaults() };
    let cases = [
        ("silent 45 min, tool sent 1 min ago", quiet(45 * MIN, MIN), true),
        ("silent 29 min: under the threshold", quiet(29 * MIN, MIN), false),
        ("tool silent too: not in use", quiet(45 * MIN, 30 * MIN), false),
        ("silent over a day: gone, not quiet", quiet(WAS_SENDING, MIN), false),
    ];
    for (case, event, notifies) in cases {
        assert_eq!(once(&event, &prefs).is_some(), notifies, "{case}");
    }
    let n = once(&quiet(45 * MIN, MIN), &prefs).unwrap();
    assert_eq!(n.title, "Claude Code telemetry has gone quiet");
    assert_eq!(n.body, "Nothing from Claude Code telemetry for 45 min while its tool is in use");
    assert_eq!((n.kind, n.target), (Kind::QuietConnector, Target::Source { id: "claude-code-otel".into() }));
}

#[test]
fn a_quiet_spell_notifies_once_and_a_new_one_again() {
    let prefs = defaults();
    let mut state = State::default();
    assert!(decide(&quiet(45 * MIN, MIN), &mut state, now(), &prefs).is_some());
    assert!(decide(&quiet(45 * MIN, MIN), &mut state, now(), &prefs).is_none(), "same spell");
    assert!(decide(&quiet(40 * MIN, MIN), &mut state, now(), &prefs).is_some(), "it sent, then went quiet again");
}

// --- switches -------------------------------------------------------------------------------

#[test]
fn each_kind_can_be_switched_off_alone() {
    let events =
        [(Kind::LongTurn, turn(30.0, false, Some("x"))), (Kind::PossibleLeak, leak()), (Kind::QuietConnector, quiet(45 * MIN, MIN))];
    let offs = [
        (Kind::LongTurn, Preferences { notify_long_turn: false, ..defaults() }),
        (Kind::PossibleLeak, Preferences { notify_leaks: false, ..defaults() }),
        (Kind::QuietConnector, Preferences { notify_quiet_connector: false, ..defaults() }),
    ];
    for (off, prefs) in &offs {
        let kinds: Vec<Kind> = events.iter().filter_map(|(_, event)| once(event, prefs)).map(|n| n.kind).collect();
        let expected: Vec<Kind> = events.iter().map(|(kind, _)| *kind).filter(|kind| kind != off).collect();
        assert_eq!(kinds, expected, "{off:?} off");
    }
}

// --- rate limit -----------------------------------------------------------------------------

#[test]
fn ten_long_turns_in_an_hour_notify_at_most_the_limit() {
    let prefs = defaults();
    let mut state = State::default();
    let sent = (0..10)
        .filter(|i| {
            let at = now().checked_add(SignedDuration::from_mins(6 * i)).unwrap();
            decide(&turn(15.0, false, Some("x")), &mut state, at, &prefs).is_some()
        })
        .count();
    assert_eq!(sent, LIMIT.max);
}

#[test]
fn the_limit_is_per_kind_and_frees_up_after_its_window() {
    let prefs = defaults();
    let mut state = State::default();
    let long = turn(15.0, false, Some("x"));
    let sent: Vec<bool> = (0..=LIMIT.max).map(|_| decide(&long, &mut state, now(), &prefs).is_some()).collect();
    assert_eq!(sent, [true, true, true, false], "the fourth in the window is held back");
    assert!(decide(&leak(), &mut state, now(), &prefs).is_some(), "another kind has its own limit");
    assert_eq!(state.sent(Kind::LongTurn, now()), LIMIT.max);
    let later = now().checked_add(LIMIT.per).unwrap();
    assert_eq!(state.sent(Kind::LongTurn, later), 0);
    assert!(decide(&long, &mut state, later, &prefs).is_some(), "a window later");
}

#[test]
fn a_notification_held_back_does_not_count() {
    let prefs = defaults();
    let mut state = State::default();
    for _ in 0..10 {
        decide(&leak(), &mut state, now(), &prefs);
    }
    assert_eq!(state.sent(Kind::PossibleLeak, now()), LIMIT.max);
}
