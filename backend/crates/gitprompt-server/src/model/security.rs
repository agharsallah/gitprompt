//! The Security page: the receiver token and its rotation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::sources::Change;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Security")]
pub struct SecurityOut {
    pub token_file: String,
    /// When the token was created or last rotated (ISO-8601).
    pub token_changed: String,
    /// Names of the connected sources (they carry the token in their config).
    pub connected: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Reconnect {
    pub source: String,
    pub name: String,
    /// The diff shows `<new token>` in a dry run.
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Rotation {
    pub reconnects: Vec<Reconnect>,
    /// Connected sources that could not be reconnected (policy); applying is then refused (409).
    pub blocked: Vec<String>,
    pub applied: bool,
}

impl From<gitprompt_capture::rotation::Rotation> for Rotation {
    fn from(r: gitprompt_capture::rotation::Rotation) -> Self {
        Rotation {
            reconnects: r.reconnects.into_iter().map(|c| Reconnect { source: c.source, name: c.name, change: c.change.into() }).collect(),
            blocked: r.blocked,
            applied: r.applied,
        }
    }
}
