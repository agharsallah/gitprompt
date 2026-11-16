//! Local usage counts: which parts of the dashboard get used, per day, on this machine only.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One feature used `n` times since the page last reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UsageHit {
    /// A dotted lowercase name such as `copy.prompt` (see `service::usage::valid_action`).
    pub action: String,
    pub n: u32,
}

/// `POST /api/usage`: what the page counted since its last report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UsageIn {
    pub hits: Vec<UsageHit>,
}

/// How many times one feature was used on one day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UsageDay {
    pub action: String,
    /// `YYYY-MM-DD`, UTC.
    pub day: String,
    pub n: u64,
}

/// `GET /api/usage?days=`: the counts of the last `days` days, today included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UsageOut {
    /// The first and last day of the range, `YYYY-MM-DD`.
    pub since: String,
    pub until: String,
    /// Oldest day first, then by action; a day with no use has no row.
    pub counts: Vec<UsageDay>,
}
