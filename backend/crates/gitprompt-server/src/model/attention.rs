//! `GET /api/attention`: computed by the insights crate, re-exported as the wire type.

pub use gitprompt_insights::attention::{
    Action as WorthAction, Attention, Item as WorthItem, Kind as WorthKind, PickUp, Presence as PickUpPresence,
};
pub use gitprompt_insights::target::Target;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_insights::notify::Notification;

/// `GET /api/recap`: the weekly recap sent for the current week (`null` when none was), for the
/// Overview's card when no desktop app shows notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Recap {
    /// The instant it was due (UTC): the week it sums up ends there, and a dismissal is per week.
    pub due: String,
    pub notification: Notification,
}
