//! Hiding one finding: what to replace in a turn's stored JSON. The clear value is found again
//! here, from the turn itself, and stays inside the [`Redactor`]: it has no public field and its
//! `Debug` does not print it.

use gitprompt_core::Json;
use gitprompt_core::json::map_strings;
use gitprompt_core::leak::Mask;
use gitprompt_store::EventRow;

use super::{finding, found};
use crate::derivations::Derived;

/// Replaces one finding's value (or, for a sensitive file, what reading it returned) in a JSON
/// value; apply it to the prompt's and its turn's events' stored JSON.
pub struct Redactor {
    target: Target,
    replacement: String,
}

enum Target {
    /// Every occurrence of this text.
    Value(String),
    /// The `tool_response` of a tool event whose `tool_input` names a file ending with this name.
    FileContents(String),
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactor").field("replacement", &self.replacement).finish_non_exhaustive()
    }
}

impl Redactor {
    /// What the value becomes: `[hidden <kind>]`.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// `json` with the target replaced (the same value when there is nothing to replace).
    pub fn apply(&self, json: &Json) -> Json {
        match &self.target {
            Target::Value(value) => map_strings(json, &|text: &str| text.replace(value.as_str(), &self.replacement)),
            Target::FileContents(name) => self.without_contents(json, name),
        }
    }

    fn without_contents(&self, json: &Json, name: &str) -> Json {
        let Json::Object(map) = json else { return json.clone() };
        let reads_it = map.get("tool_input").is_some_and(|input| strings_end_with(input, name));
        if !reads_it || !map.contains_key("tool_response") {
            return json.clone();
        }
        let mut map = map.clone();
        map.insert("tool_response".into(), Json::String(self.replacement.clone()));
        Json::Object(map)
    }
}

/// Some string in `value` is a path ending with `name`.
fn strings_end_with(value: &Json, name: &str) -> bool {
    match value {
        Json::String(text) => text.ends_with(name),
        Json::Array(items) => items.iter().any(|v| strings_end_with(v, name)),
        Json::Object(map) => map.values().any(|v| strings_end_with(v, name)),
        _ => false,
    }
}

/// The redactor of finding `id` of this turn, if the turn still holds it.
pub fn redactor(derived: &Derived, events: &[EventRow], id: &str) -> Option<Redactor> {
    let turn = derived.prompt.row.id;
    let (_, found) = found(derived, events).into_iter().find(|(place, found)| finding(turn, *place, *found).id == id)?;
    let target =
        if found.mask == Mask::Plain { Target::FileContents(found.value.to_owned()) } else { Target::Value(found.value.to_owned()) };
    Some(Redactor { target, replacement: format!("[hidden {}]", found.kind) })
}
