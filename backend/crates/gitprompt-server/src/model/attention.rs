//! `GET /api/attention`: computed by the insights crate, re-exported as the wire type.

pub use gitprompt_insights::attention::{
    Action as WorthAction, Attention, Item as WorthItem, Kind as WorthKind, PickUp, Presence as PickUpPresence,
};
pub use gitprompt_insights::target::Target;
