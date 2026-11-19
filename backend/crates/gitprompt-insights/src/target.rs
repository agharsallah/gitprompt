//! Where something the daemon points you at opens in the dashboard: a Worth a look row, a
//! notification, the weekly recap. Data, not URLs: the dashboard builds the link (`lib/shared/links.ts`),
//! the tray and notifications map it to a route.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::preferences::RangeId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "page", rename_all = "snake_case")]
#[ts(rename = "AttentionTarget")]
pub enum Target {
    /// The Overview, on `range` when given (else the range it opens with).
    Overview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        range: Option<RangeId>,
    },
    /// The Prompts page, finding these words.
    Prompts { q: String },
    /// One prompt's turn in the reader.
    Prompt { id: i64 },
    /// One session's story.
    Session { id: String },
    /// The possible leaks of the Overview.
    Leaks,
    /// One connector's card on the Connectors page.
    Source { id: String },
}
