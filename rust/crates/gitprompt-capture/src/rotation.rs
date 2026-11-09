//! Rotating the receiver token: a new secret in `<home>/token`, then every connected source is
//! reconnected so its entries carry the new one. Port of `sources/rotation.py`.
//!
//! Same consent rule as Connect: a dry run returns, per connected source, the exact diff that would
//! be written (the new token shown as [`PLACEHOLDER`]: it doesn't exist yet). Rotation is refused
//! when a connected source can't be reconnected (blocked by policy, unreadable config), because that
//! source would silently stop being captured.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use gitprompt_core::spec::Spec;
use gitprompt_core::{Result, Settings};

use crate::connection::Change;

/// Stands for the new token in a dry run's diffs.
pub const PLACEHOLDER: &str = "<new token>";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconnect {
    pub source: String,
    pub name: String,
    pub change: Change,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotation {
    /// One per connected source, in spec id order.
    pub reconnects: Vec<Reconnect>,
    /// `"<name>: <blocks>"` of connected sources that could not be reconnected.
    pub blocked: Vec<String>,
    pub applied: bool,
}

/// Dry run: the blocked list (and no reconnects) if any connected source is blocked by policy,
/// else each connected source's reconnect diff with [`PLACEHOLDER`]; nothing changes.
/// Applied: `Error::PolicyBlocked` ("Can't reconnect ... Disconnect it first, then rotate.") when one
/// is blocked, and a config that can't be parsed fails before the token changes; otherwise the token
/// is rotated (`Settings::rotate_token`) and every connected source reconnected with it.
pub fn rotate(_specs: &BTreeMap<String, Arc<Spec>>, _settings: &Settings, _dry_run: bool) -> Result<Rotation> {
    todo!()
}
