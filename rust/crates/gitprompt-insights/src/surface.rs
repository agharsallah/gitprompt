//! Where a prompt was typed: the app or client of its tool (CLI, VS Code, a desktop app...), apart
//! from the connector that delivered it. Derived on read from the source spec's `surface` rules
//! (ADR-0022), so a new rule applies to old prompts too. A rule has the conditions of an origin rule
//! ([`crate::origin::matches`], with no typed text). Unknown stays `None`: a surface is never
//! guessed. Pure functions, no tool knowledge.

use indexmap::IndexMap;

use gitprompt_core::Json;
use gitprompt_core::spec::{Spec, SurfaceInfo};
use gitprompt_store::{EventRow, PromptRow};

/// What the derived value means (listed on the Fields page).
pub const DOCS: &[(&str, &str)] =
    &[("surface", "where the prompt was typed: the app or client of its tool (cli, vscode, desktop...); null when unknown")];

/// The surface of the first rule of the spec that holds for this prompt and its turn; `None` without
/// a spec or when no rule matches.
pub fn surface_of(_prompt: &PromptRow, _events: &[EventRow], _spec: Option<&Spec>) -> Option<String> {
    todo!()
}

/// Each payload path the spec's surface rules read -> the value this prompt (or, for a rule with an
/// `event`, the first event of that name in its turn that has one) holds there, in rule order.
/// Paths without a value are left out; a path read by several rules appears once. Shows why a
/// prompt got its surface, or what an unknown client reported (`gitprompt check`).
pub fn signals(_prompt: &PromptRow, _events: &[EventRow], _spec: Option<&Spec>) -> IndexMap<String, Json> {
    todo!()
}

/// Every surface any spec names (the shared ones of `_defaults.yaml`, and each spec's own); a later
/// spec's entry replaces an earlier one with the same id.
pub fn catalog<'a>(_specs: impl IntoIterator<Item = &'a Spec>) -> IndexMap<String, SurfaceInfo> {
    todo!()
}
