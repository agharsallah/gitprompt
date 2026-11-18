//! `GET /api/leaks`: the possible leaks of a range, open first, with what you decided about the
//! others. A value you marked not sensitive is dismissed everywhere it shows up; a value on the
//! allow-list (your git email, `leak_allow`) is never shown.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_store::leaks::{LeakState, StoredLeak};

use super::Place;
use super::key::KINDS_CHECKED;

/// What you decided about a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LeakDecision {
    /// Its value was replaced in the stored text.
    Hidden,
    /// Not a leak: dismissed, wherever the same value appears.
    NotSensitive,
}

impl LeakDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            LeakDecision::Hidden => "hidden",
            LeakDecision::NotSensitive => "not_sensitive",
        }
    }

    pub fn parse(name: &str) -> Option<LeakDecision> {
        [LeakDecision::Hidden, LeakDecision::NotSensitive].into_iter().find(|d| d.as_str() == name)
    }
}

/// One finding as the dashboard shows it: masked, never the value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LeakOut {
    pub id: String,
    pub kind: String,
    pub masked: String,
    pub place: Place,
    /// The prompt whose turn holds it.
    pub prompt: i64,
    /// When that prompt was received (a hidden finding: when it was hidden).
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decided: Option<LeakDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decided_at: Option<String>,
}

/// What was looked through: "1,204 prompts checked for 26 kinds".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Checked {
    pub prompts: i64,
    pub kinds: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Leaks {
    /// Not decided yet, newest first.
    pub open: Vec<LeakOut>,
    /// Values you hid (whatever the range: the value is gone from the text).
    pub hidden: Vec<LeakOut>,
    /// Findings you marked not sensitive, and the same values elsewhere.
    pub dismissed: Vec<LeakOut>,
    pub checked: Checked,
}

/// The board of `found` (newest first) given your decisions and the allow-list's value keys.
pub fn board(found: Vec<StoredLeak>, states: &[LeakState], allow: &BTreeSet<String>, prompts: i64) -> Leaks {
    let by_finding: HashMap<&str, &LeakState> = states.iter().map(|s| (s.finding.as_str(), s)).collect();
    let dismissed_values: HashMap<&str, &LeakState> =
        states.iter().filter(|s| s.state == LeakDecision::NotSensitive.as_str()).map(|s| (s.value_key.as_str(), s)).collect();
    let mut out = Leaks { checked: Checked { prompts, kinds: KINDS_CHECKED }, ..Leaks::default() };
    for leak in found.into_iter().filter(|l| !allow.contains(&l.value_key)) {
        let decision = by_finding.get(leak.finding.as_str()).or_else(|| dismissed_values.get(leak.value_key.as_str()));
        match decision.and_then(|state| Some((LeakDecision::parse(&state.state)?, state.at.clone()))) {
            Some((LeakDecision::NotSensitive, at)) => out.dismissed.push(shown(leak, Some((LeakDecision::NotSensitive, at)))),
            Some((LeakDecision::Hidden, _)) => {}
            None => out.open.push(shown(leak, None)),
        }
    }
    out.hidden = states.iter().filter(|s| s.state == LeakDecision::Hidden.as_str()).map(hidden).collect();
    out.hidden.reverse();
    out
}

fn shown(leak: StoredLeak, decided: Option<(LeakDecision, String)>) -> LeakOut {
    let (decided, decided_at) = decided.map_or((None, None), |(d, at)| (Some(d), Some(at)));
    LeakOut {
        id: leak.finding,
        kind: leak.kind,
        masked: leak.masked,
        place: Place::parse(&leak.place).unwrap_or(Place::Prompt),
        prompt: leak.prompt,
        at: leak.received_at,
        session: leak.session,
        decided,
        decided_at,
    }
}

fn hidden(state: &LeakState) -> LeakOut {
    LeakOut {
        id: state.finding.clone(),
        kind: state.kind.clone(),
        masked: state.masked.clone(),
        place: Place::parse(&state.place).unwrap_or(Place::Prompt),
        prompt: state.prompt,
        at: state.at.clone(),
        session: None,
        decided: Some(LeakDecision::Hidden),
        decided_at: Some(state.at.clone()),
    }
}
