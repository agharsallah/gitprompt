//! The two keys of a finding: its id (kind, turn, place, value) and its value key (kind and value),
//! digests from which the value cannot be read back casually, stable across versions and machines.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use gitprompt_core::personal::{DETECTORS, PRIVATE_TERM, SENSITIVE_FILES};
use gitprompt_core::secrets::KINDS;

use super::Place;

/// How many kinds are looked for: what "All clear" says was checked.
pub const KINDS_CHECKED: usize = KINDS.len() + DETECTORS.len() + SENSITIVE_FILES.len() + 1;

/// Every kind's name ([`PRIVATE_TERM`] included).
fn kind_names() -> impl Iterator<Item = &'static str> {
    KINDS
        .iter()
        .map(|(name, _)| *name)
        .chain(DETECTORS.iter().map(|d| d.name))
        .chain(SENSITIVE_FILES.iter().map(|(name, _)| *name))
        .chain([PRIVATE_TERM])
}

/// The same value of the same kind wherever it appears (spaces and case aside).
pub fn value_key(kind: &str, value: &str) -> String {
    digest(&[kind, &normal(value)])
}

/// One finding: its kind, the turn, where in the turn, and its value (through its key).
pub(super) fn finding_id(kind: &str, turn: i64, place: Place, value_key: &str) -> String {
    digest(&[kind, &turn.to_string(), place.as_str(), value_key])
}

/// The value keys of values that are never a leak (your git email, the `leak_allow` list), for
/// every kind, since a value says nothing of the kind it would be found as.
pub fn allow_keys(values: &[String]) -> BTreeSet<String> {
    values.iter().filter(|v| !v.trim().is_empty()).flat_map(|value| kind_names().map(move |kind| value_key(kind, value.trim()))).collect()
}

/// The value as compared: no spaces, any case (`DE44 5001` is `de445001`).
fn normal(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

/// The first 16 hex digits of the SHA-256 of the parts, each ended by a zero byte.
fn digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().iter().take(8).fold(String::with_capacity(16), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
