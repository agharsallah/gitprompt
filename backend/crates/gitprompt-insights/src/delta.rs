//! How each key figure compares with the period before (the Overview's headline and figure tiles).
//!
//! A change is a ratio, never a guess: without a "before" (the period before held no prompt, as in
//! a first week) or with a "before" of 0, there is no change to state, so `change` is absent rather
//! than +∞ or a misleading +100%.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::numbers::round;
use gitprompt_store::stats::{Bucket, Period};

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

/// The key figures the Overview leads with, each against the period before. Phase 3 adds redone
/// work and possible leaks here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct KeyChanges {
    pub prompts: Delta,
    pub sessions: Delta,
    pub cost_usd: Delta,
    /// Agent working time, seconds.
    pub agent_time_s: Delta,
    /// Commits made during the turns.
    pub git_commits: Delta,
}

/// Whole-number counts as figures.
#[expect(clippy::cast_precision_loss, reason = "prompt and session counts are far below 2^52")]
fn count(n: i64) -> f64 {
    n as f64
}

impl KeyChanges {
    /// The figures of `now` against `before`; `before` holding no prompt counts as no "before".
    pub fn between(now: &Bucket, before: &Bucket) -> KeyChanges {
        let had = before.prompts > 0;
        let delta = |figure: fn(&Bucket) -> f64| Delta::new(figure(now), had.then(|| figure(before)));
        KeyChanges {
            prompts: delta(|b| count(b.prompts)),
            sessions: delta(|b| count(b.sessions)),
            cost_usd: delta(|b| b.cost_usd),
            agent_time_s: delta(|b| b.agent_time_s),
            git_commits: delta(|b| b.git_commits),
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
    pub changes: KeyChanges,
}

impl Previous {
    pub fn new(period: Period, now: &Bucket, totals: Bucket) -> Previous {
        let changes = KeyChanges::between(now, &totals);
        Previous { since: period.since, until: period.until, totals, changes }
    }
}
