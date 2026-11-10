//! Where a prompt was typed: the app or client of its tool (CLI, VS Code, a desktop app...), apart
//! from the connector that delivered it. Derived on read from the source spec's `surface` rules
//! (ADR-0022), so a new rule applies to old prompts too. A rule has the conditions of an origin rule
//! ([`crate::origin::matches`], with no typed text). Unknown stays `None`: a surface is never
//! guessed. Pure functions, no tool knowledge.

use indexmap::IndexMap;

use gitprompt_core::Json;
use gitprompt_core::json::{is_no_value, pluck};
use gitprompt_core::spec::{Spec, SurfaceInfo};
use gitprompt_store::{EventRow, PromptRow};

use crate::origin::matches_in;
use crate::raw::{self, Event, Raw};

/// What the derived value means (listed on the Fields page).
pub const DOCS: &[(&str, &str)] =
    &[("surface", "where the prompt was typed: the app or client of its tool (cli, vscode, desktop...); null when unknown")];

/// The surface of the first rule of the spec that holds for this prompt and its turn; `None` without
/// a spec or when no rule matches.
pub fn surface_of(prompt: &PromptRow, events: &[EventRow], spec: Option<&Spec>) -> Option<String> {
    surface_in(prompt, &Raw::new(&prompt.raw, None), &raw::events(events, None), spec)
}

/// [`surface_of`] over payloads being read (and maybe recorded).
pub(crate) fn surface_in(prompt: &PromptRow, raw: &Raw<'_>, events: &[Event<'_>], spec: Option<&Spec>) -> Option<String> {
    spec?.surface.iter().find(|rule| matches_in(&rule.when, prompt, raw, events, "")).map(|rule| rule.surface.clone())
}

/// Each payload path the spec's surface rules read -> the value this prompt (or, for a rule with an
/// `event`, the first event of that name in its turn that has one) holds there, in rule order.
/// Paths without a value are left out; a path read by several rules appears once. Shows why a
/// prompt got its surface, or what an unknown client reported (`gitprompt check`).
pub fn signals(prompt: &PromptRow, events: &[EventRow], spec: Option<&Spec>) -> IndexMap<String, Json> {
    let mut out = IndexMap::new();
    for rule in spec.map_or(&[][..], |spec| &spec.surface) {
        let Some(path) = rule.when.path.as_deref().filter(|path| !path.is_empty() && !out.contains_key(*path)) else { continue };
        let payloads: Vec<&Json> = match &rule.when.event {
            Some(name) => events.iter().filter(|e| e.event.as_deref() == Some(name.as_str())).map(|e| &e.raw).collect(),
            None => vec![&prompt.raw],
        };
        if let Some(value) = payloads.into_iter().find_map(|payload| pluck(payload, path).filter(|v| !is_no_value(v))) {
            out.insert(path.to_owned(), value.clone());
        }
    }
    out
}

/// Every surface any spec names (the shared ones of `_defaults.yaml`, and each spec's own); a later
/// spec's entry replaces an earlier one with the same id.
pub fn catalog<'a>(specs: impl IntoIterator<Item = &'a Spec>) -> IndexMap<String, SurfaceInfo> {
    let mut out = IndexMap::new();
    for spec in specs {
        out.extend(spec.surfaces.iter().map(|(id, info)| (id.clone(), info.clone())));
    }
    out
}
