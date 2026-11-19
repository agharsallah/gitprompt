//! What deserves a notification, decided in the daemon so it works with the window closed and
//! stays quiet otherwise. Pure: the daemon gathers [`Event`]s, keeps the [`State`] and passes
//! "now" in, so every rule and the rate limit are tested without a clock.
//!
//! One rule per [`Kind`] ([`rules`]), each with its switch and threshold in the preferences, then a
//! rate limit per kind ([`LIMIT`]): a burst of long turns makes a few notifications, not ten.

mod rules;

use std::collections::BTreeMap;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::preferences::Preferences;

use crate::target::Target;

/// Notifications of one kind sent at most `max` per `per`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub max: usize,
    pub per: SignedDuration,
}

/// The rate limit of every kind: three an hour.
pub const LIMIT: Limit = Limit { max: 3, per: SignedDuration::from_hours(1) };

/// A connector silent for longer than this is gone, not newly quiet: it is never notified.
pub const WAS_SENDING: SignedDuration = SignedDuration::from_hours(24);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "NotificationKind")]
pub enum Kind {
    /// A turn that ran long finished or failed.
    LongTurn,
    /// A new possible leak in what was captured.
    PossibleLeak,
    /// A connector that was sending went quiet while its tool is in use.
    QuietConnector,
    /// Once a week: what the week cost, what was redone, what was committed ([`crate::recap`]).
    WeeklyRecap,
}

/// One notification: what the desktop shell shows and the dashboard receives on its live stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Notification {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// Where clicking it leads.
    pub target: Target,
}

/// A turn that ended (finished or failed; an interrupted turn is never notified: you stopped it).
#[derive(Clone, Debug, PartialEq)]
pub struct TurnEnd {
    pub prompt: i64,
    pub failed: bool,
    /// How long it ran.
    pub seconds: f64,
    /// What was typed, if anything (its label names the turn).
    pub text: Option<String>,
    /// The connector's name, naming a turn with no typed text.
    pub tool: String,
}

/// A possible leak found since the last check (masked: never the value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeakFound {
    pub kind: String,
    pub masked: String,
}

/// A connector and the newest arrival from the other connectors of its tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectorSeen {
    pub id: String,
    pub name: String,
    /// Its newest arrival.
    pub last_at: Timestamp,
    /// The newest arrival from the other connectors of the same tool: the tool is in use.
    pub tool_last_at: Timestamp,
}

/// Something that may deserve a notification.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    TurnEnded(TurnEnd),
    LeakFound(LeakFound),
    ConnectorSeen(ConnectorSeen),
}

/// What the notifier remembers between events: when each kind last notified (the rate limit), and
/// the quiet spell of each connector already notified (by its last arrival).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    sent: BTreeMap<Kind, Vec<Timestamp>>,
    quiet: BTreeMap<String, Timestamp>,
}

impl State {
    /// Notifications of `kind` sent within [`LIMIT`] before `now`.
    pub fn sent(&self, kind: Kind, now: Timestamp) -> usize {
        self.sent.get(&kind).map_or(0, |times| times.iter().filter(|at| within(**at, now)).count())
    }

    fn record(&mut self, event: &Event, kind: Kind, now: Timestamp) {
        let times = self.sent.entry(kind).or_default();
        times.retain(|at| within(*at, now));
        times.push(now);
        if let Event::ConnectorSeen(seen) = event {
            self.quiet.insert(seen.id.clone(), seen.last_at);
        }
    }

    /// Whether this quiet spell of the connector was notified already.
    fn notified_quiet(&self, seen: &ConnectorSeen) -> bool {
        self.quiet.get(&seen.id) == Some(&seen.last_at)
    }
}

/// Whether `at` counts against the limit at `now`.
fn within(at: Timestamp, now: Timestamp) -> bool {
    now.duration_since(at) < LIMIT.per
}

/// The notification `event` deserves at `now`, if any, and `state` updated: `None` when its kind is
/// switched off, it is under its threshold, it was notified already or the kind's rate limit is
/// reached (it is then dropped, not queued).
pub fn decide(event: &Event, state: &mut State, now: Timestamp, prefs: &Preferences) -> Option<Notification> {
    let notification = rules::rule(event, state, now, prefs)?;
    if state.sent(notification.kind, now) >= LIMIT.max {
        return None;
    }
    state.record(event, notification.kind, now);
    Some(notification)
}
