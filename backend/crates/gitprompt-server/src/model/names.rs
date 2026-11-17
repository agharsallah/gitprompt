//! The names you give things: today a session's.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `PUT /api/names/session/{id}`: what to call it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NameIn {
    /// Trimmed, white space collapsed, 1 to 80 characters, no control characters.
    pub name: String,
}

/// The name as stored (cleaned), which may differ from what was sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NameOut {
    pub name: String,
}
