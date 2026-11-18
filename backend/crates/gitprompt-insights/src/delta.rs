//! How each key figure compares with the period before (the Overview's headline and figure tiles).
//!
//! A change is a ratio, never a guess: without a "before" (the period before held no prompt, as in
//! a first week) or with a "before" of 0, there is no change to state, so `change` is absent rather
//! than +∞ or a misleading +100%.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::numbers::round;
use gitprompt_store::stats::{Bucket, Period};

use crate::redone::RedoneWork;

/// One figure now and in the period before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Delta {
    pub now: f64,
    /// Absent when the period before held no prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub before: Option<f64>,
    /// `(now − before) / before`, 3 decimals (0.18 is +18%); absent without a `before`, or when it
    /// is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub change: Option<f64>,
}

impl Delta {
    /// `now` against `before` (None: nothing to compare with).
    pub fn new(now: f64, before: Option<f64>) -> Delta {
        let change = before.filter(|b| *b != 0.0 && b.is_finite()).map(|b| round((now - b) / b, 3));
        Delta { now, before, change }
    }
}

/// What a period's key figures are read from: its totals and, when asked for, its redone work.
#[derive(Clone, Copy, Debug)]
pub struct Figures<'a> {
    pub totals: &'a Bucket,
    pub redone: Option<&'a RedoneWork>,
}

/// The key figures the Overview leads with, each against the period before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct KeyChanges {
    pub prompts: Delta,
    pub sessions: Delta,
    pub cost_usd: Delta,
    /// Agent working time, seconds.
    pub agent_time_s: Delta,
    /// Commits made during the turns.
    pub git_commits: Delta,
    /// Agent time of the turns you redid, seconds; with `key_figures`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub redone_agent_time_s: Option<Delta>,
    /// Their cost; only where tools report cost now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub redone_cost_usd: Option<Delta>,
}

/// Whole-number counts as figures.
#[expect(clippy::cast_precision_loss, reason = "prompt and session counts are far below 2^52")]
fn count(n: i64) -> f64 {
    n as f64
}

impl KeyChanges {
    /// The figures of `now` against `before`; `before` holding no prompt counts as no "before".
    pub fn between(now: Figures<'_>, before: Figures<'_>) -> KeyChanges {
        let had = before.totals.prompts > 0;
        let delta = |figure: fn(&Bucket) -> f64| Delta::new(figure(now.totals), had.then(|| figure(before.totals)));
        let redone = |figure: fn(&RedoneWork) -> Option<f64>| {
            let value = figure(now.redone?)?;
            Some(Delta::new(value, if had { before.redone.and_then(figure) } else { None }))
        };
        KeyChanges {
            prompts: delta(|b| count(b.prompts)),
            sessions: delta(|b| count(b.sessions)),
            cost_usd: delta(|b| b.cost_usd),
            agent_time_s: delta(|b| b.agent_time_s),
            git_commits: delta(|b| b.git_commits),
            redone_agent_time_s: redone(|r| Some(r.total.agent_time_s)),
            redone_cost_usd: redone(|r| r.total.cost_usd),
        }
    }
}

/// The period before the requested one: its span, its totals, and the key figures compared.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "PreviousPeriod")]
pub struct Previous {
    /// UTC, inclusive.
    pub since: String,
    /// UTC, exclusive: where the requested period starts.
    pub until: String,
    pub totals: Bucket,
    /// With `key_figures`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub redone: Option<RedoneWork>,
    pub changes: KeyChanges,
}

impl Previous {
    /// The period before, its totals and redone work, compared with `now`.
    pub fn new(period: Period, now: Figures<'_>, totals: Bucket, redone: Option<RedoneWork>) -> Previous {
        let changes = KeyChanges::between(now, Figures { totals: &totals, redone: redone.as_ref() });
        Previous { since: period.since, until: period.until, totals, redone, changes }
    }
}
