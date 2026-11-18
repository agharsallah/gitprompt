//! One rule per kind: its switch, its threshold, its words.

use jiff::{SignedDuration, Timestamp};

use gitprompt_core::preferences::Preferences;

use super::{ConnectorSeen, Event, Kind, LeakFound, Notification, State, TurnEnd, WAS_SENDING};
use crate::label::label;
use crate::target::Target;

/// The notification of `event` before the rate limit, if its rule wants one.
pub(super) fn rule(event: &Event, state: &State, now: Timestamp, prefs: &Preferences) -> Option<Notification> {
    match event {
        Event::TurnEnded(turn) => long_turn(turn, prefs),
        Event::LeakFound(leak) => possible_leak(leak, prefs),
        Event::ConnectorSeen(seen) => quiet_connector(seen, state, now, prefs),
    }
}

fn minutes(minutes: u32) -> SignedDuration {
    SignedDuration::from_mins(i64::from(minutes))
}

/// A turn that ran at least `notify_long_turn_minutes`.
fn long_turn(turn: &TurnEnd, prefs: &Preferences) -> Option<Notification> {
    let ran = SignedDuration::try_from_secs_f64(turn.seconds).ok()?;
    if !prefs.notify_long_turn || ran < minutes(prefs.notify_long_turn_minutes) {
        return None;
    }
    let name = turn.text.as_deref().and_then(label).unwrap_or_else(|| format!("A {} turn", turn.tool));
    let (title, body) = if turn.failed {
        ("A long turn failed", format!("{name} failed after {}", spoken(ran)))
    } else {
        ("A long turn finished", format!("{name} ran for {}", spoken(ran)))
    };
    Some(Notification { kind: Kind::LongTurn, title: title.to_owned(), body, target: Target::Prompt { id: turn.prompt } })
}

/// Every new possible leak, masked.
fn possible_leak(leak: &LeakFound, prefs: &Preferences) -> Option<Notification> {
    prefs.notify_leaks.then(|| Notification {
        kind: Kind::PossibleLeak,
        title: "A possible leak".to_owned(),
        body: format!("{} {} was captured", leak.kind, leak.masked),
        target: Target::Leaks,
    })
}

/// A connector silent for `notify_quiet_connector_minutes` (but not for a day: then it is gone)
/// while another connector of its tool sent within that time, once per quiet spell.
fn quiet_connector(seen: &ConnectorSeen, state: &State, now: Timestamp, prefs: &Preferences) -> Option<Notification> {
    let threshold = minutes(prefs.notify_quiet_connector_minutes);
    let silent = now.duration_since(seen.last_at);
    let quiet = silent >= threshold && silent < WAS_SENDING;
    let in_use = now.duration_since(seen.tool_last_at) < threshold;
    if !prefs.notify_quiet_connector || !quiet || !in_use || state.notified_quiet(seen) {
        return None;
    }
    Some(Notification {
        kind: Kind::QuietConnector,
        title: format!("{} has gone quiet", seen.name),
        body: format!("Nothing from {} for {} while its tool is in use", seen.name, spoken(silent)),
        target: Target::Source { id: seen.id.clone() },
    })
}

/// `12 min`, `1 h 05 min` (whole minutes, at least 1).
fn spoken(duration: SignedDuration) -> String {
    let total = duration.as_mins().max(1);
    let (hours, mins) = (total / 60, total % 60);
    if hours == 0 { format!("{mins} min") } else { format!("{hours} h {mins:02} min") }
}
