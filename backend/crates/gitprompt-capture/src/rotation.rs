//! Rotating the receiver token: a new secret in `<home>/token`, then every connected source is
//! reconnected so its entries carry the new one. Port of `sources/rotation.py`.
//!
//! Same consent rule as Connect: a dry run returns, per connected source, the exact diff that would
//! be written (the new token shown as [`PLACEHOLDER`]: it doesn't exist yet). Rotation is refused
//! when a connected source can't be reconnected (blocked by policy, unreadable config), because that
//! source would silently stop being captured.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use gitprompt_core::spec::Spec;
use gitprompt_core::{Error, Result, Settings};

use crate::connection::{Change, Connection};
use crate::probe::check_policy;

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
pub fn rotate(specs: &BTreeMap<String, Arc<Spec>>, settings: &Settings, dry_run: bool) -> Result<Rotation> {
    rotate_via(specs, settings, dry_run, None)
}

/// [`rotate`], reconnecting with hook commands that run `forwarder` (see
/// [`Connection::with_forwarder`]); `None` = the spec's curl commands.
pub fn rotate_via(specs: &BTreeMap<String, Arc<Spec>>, settings: &Settings, dry_run: bool, forwarder: Option<&Path>) -> Result<Rotation> {
    let connected: Vec<&Spec> =
        specs.values().map(Arc::as_ref).filter(|spec| connection_of(spec, settings, forwarder).connected()).collect();
    let blocked: Vec<String> = connected
        .iter()
        .filter_map(|spec| {
            let blocks = check_policy(spec, settings).blocks;
            (!blocks.is_empty()).then(|| format!("{}: {}", spec.name, blocks.join("; ")))
        })
        .collect();
    if !blocked.is_empty() {
        if dry_run {
            return Ok(Rotation { blocked, ..Rotation::default() });
        }
        return Err(Error::PolicyBlocked(format!("Can't reconnect {}. Disconnect it first, then rotate.", blocked.join("; "))));
    }
    // Preview first, also before applying: an unreadable config fails here, before the token changes.
    let reconnect = |spec: &Spec, connection: Connection<'_>, dry_run: bool| -> Result<Reconnect> {
        Ok(Reconnect { source: spec.id.clone(), name: spec.name.clone(), change: connection.connect(dry_run)? })
    };
    let preview = connected
        .iter()
        .map(|spec| reconnect(spec, connection_of(spec, settings, forwarder).with_token(PLACEHOLDER), true))
        .collect::<Result<Vec<_>>>()?;
    if dry_run {
        return Ok(Rotation { reconnects: preview, ..Rotation::default() });
    }
    settings.rotate_token()?;
    let reconnects =
        connected.iter().map(|spec| reconnect(spec, connection_of(spec, settings, forwarder), false)).collect::<Result<Vec<_>>>()?;
    Ok(Rotation { reconnects, blocked: Vec::new(), applied: true })
}

fn connection_of<'a>(spec: &'a Spec, settings: &'a Settings, forwarder: Option<&Path>) -> Connection<'a> {
    Connection::new(spec, settings).via(forwarder)
}
