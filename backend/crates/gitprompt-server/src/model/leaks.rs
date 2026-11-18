//! `GET /api/leaks` (computed by the insights crate) and `PUT /api/leaks/{id}`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use gitprompt_insights::leaks::Place as LeakPlace;
pub use gitprompt_insights::leaks::{Checked as LeaksChecked, LeakDecision, LeakOut, Leaks};

/// Body of `PUT /api/leaks/{id}`: `hidden` replaces the value in the stored text (it cannot be
/// undone), `not_sensitive` dismisses it wherever the same value appears, `null` opens it again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LeakIn {
    pub state: Option<LeakDecision>,
}

/// Answer of `PUT /api/leaks/{id}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LeakSaved {
    pub id: String,
    pub state: Option<LeakDecision>,
    /// Stored rows rewritten (hiding only).
    pub updated: u64,
}
