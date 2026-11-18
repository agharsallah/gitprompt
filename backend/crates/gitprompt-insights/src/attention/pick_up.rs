//! Pick up where you left off: sessions still running, then the ones you left, each with how its
//! latest turn ended and the command that resumes it. Always the latest sessions, whatever range
//! the page shows.

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::spec::Resume;
use gitprompt_store::filter::Outcome;

/// Sessions Pick up shows at most.
pub const PICK_UP: usize = 3;

/// A session whose last event is younger than this and whose latest turn has no end is running
/// (the dashboard's `ACTIVE_WINDOW_MS`, so the Overview and a session card agree).
pub const RUNNING_WINDOW: SignedDuration = SignedDuration::from_mins(10);

/// A session left longer ago than this is no longer one to pick up.
pub const PICK_UP_WITHIN: SignedDuration = SignedDuration::from_hours(7 * 24);

/// What the daemon knows about one recent session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionState {
    pub id: String,
    /// What you called it.
    pub name: Option<String>,
    /// Its default name.
    pub label: Option<String>,
    /// The owner/name of its remote.
    pub project: Option<String>,
    /// When its latest event or prompt arrived (ISO-8601).
    pub last_event_at: Option<String>,
    /// Its latest prompt and how that turn ended.
    pub latest_prompt: Option<i64>,
    pub ended: Option<Outcome>,
    pub resume: Option<Resume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "PickUpPresence")]
pub enum Presence {
    Running,
    Left,
}

/// One row of Pick up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct PickUp {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    pub presence: Presence,
    /// How its latest turn ended (`running` or `open` when it has no end).
    pub ended: Outcome,
    pub last_event_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub latest_prompt: Option<i64>,
    /// Omitted when its tool has no resume command or its folder is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub resume: Option<Resume>,
}

/// A turn that has an end: the tool is waiting for you.
fn has_end(outcome: Outcome) -> bool {
    matches!(outcome, Outcome::Done | Outcome::Failed | Outcome::Interrupted)
}

/// The rows of `sessions` at `now`: running first, then left, each newest first, at most
/// [`PICK_UP`]. Sessions without a time, or left longer than [`PICK_UP_WITHIN`] ago, are skipped.
pub fn pick_up(sessions: &[SessionState], now: Timestamp) -> Vec<PickUp> {
    let mut rows: Vec<(Timestamp, PickUp)> = sessions.iter().filter_map(|session| row(session, now)).collect();
    rows.sort_by(|(a_at, a), (b_at, b)| a.presence.cmp(&b.presence).then(b_at.cmp(a_at)).then(a.session.cmp(&b.session)));
    rows.into_iter().take(PICK_UP).map(|(_, row)| row).collect()
}

fn row(session: &SessionState, now: Timestamp) -> Option<(Timestamp, PickUp)> {
    let last_event_at = session.last_event_at.clone()?;
    let at: Timestamp = last_event_at.parse().ok()?;
    let age = now.duration_since(at);
    if age > PICK_UP_WITHIN {
        return None;
    }
    let ended = session.ended.unwrap_or(Outcome::Open);
    let presence = if !has_end(ended) && age < RUNNING_WINDOW { Presence::Running } else { Presence::Left };
    let row = PickUp {
        session: session.id.clone(),
        name: session.name.clone(),
        label: session.label.clone(),
        project: session.project.clone(),
        presence,
        ended,
        last_event_at,
        latest_prompt: session.latest_prompt,
        resume: session.resume.clone(),
    };
    Some((at, row))
}
