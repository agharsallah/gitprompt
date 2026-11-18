//! Where something the daemon points you at opens in the dashboard: a Worth a look row, a
//! notification, the weekly recap. Data, not URLs: the dashboard builds the link (`lib/shared/links.ts`),
//! the tray and notifications map it to a route.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "page", rename_all = "snake_case")]
#[ts(rename = "AttentionTarget")]
pub enum Target {
    /// The Overview.
    Overview,
    /// The Prompts page, finding these words.
    Prompts { q: String },
    /// One prompt's turn in the reader.
    Prompt { id: i64 },
    /// One session's story.
    Session { id: String },
    /// The possible leaks of the Overview.
    Leaks,
}
