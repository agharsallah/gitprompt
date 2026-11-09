//! Field catalog (the Fields page): every field seen in stored payloads, next to the fields each tool
//! documents, so features can be chosen from real data.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use gitprompt_core::Json;
use gitprompt_core::spec::Spec;
use gitprompt_store::store::Sample;

/// Characters of a string sample kept.
pub const SAMPLE_CHARS: usize = 200;
/// In a spec's `documented`: fields every event of the source carries.
pub const ANY_EVENT: &str = "*";

/// One (source, event, path).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub source: String,
    pub event: String,
    /// Flattened path (`gitprompt_core::json::flatten`: `a.b`, list items as `a[]`).
    pub path: String,
    /// Values seen at this path (each item of a list counts); 0 for a documented field not seen yet.
    pub count: u64,
    /// The last non-empty value seen (strings cut to [`SAMPLE_CHARS`] characters).
    pub sample: Option<Json>,
    /// Some `documented` name of its spec (for its event or [`ANY_EVENT`]) covers it: the name
    /// itself or anything nested under it (`name.x`, `name[]`).
    pub documented: bool,
    /// The spec's `ignored` reason when an ignored name covers it: GitPrompt deliberately leaves it
    /// unused.
    pub unused: Option<String>,
}

/// An event (or data) a source can't or doesn't capture, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotCaptured {
    pub source: String,
    pub event: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    /// Sorted by (source, event, path).
    pub fields: Vec<Field>,
    /// Each spec's `not_captured`, then its `drop_events`, in spec order.
    pub not_captured: Vec<NotCaptured>,
    /// [`crate::derivations::docs`].
    pub derived: IndexMap<String, String>,
}

/// Fields seen in `samples`, plus every documented field not seen yet (count 0, under its
/// documented event name, `*` included), with `documented` and `unused` marked from the specs.
pub fn build_catalog<'a>(_samples: &[Sample], _specs: impl IntoIterator<Item = &'a Spec>) -> Catalog {
    todo!()
}
