//! Field catalog (the Fields page): every field seen in stored payloads, next to the fields each tool
//! documents, so features can be chosen from real data.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::Json;
use gitprompt_core::json::{flatten, is_no_value};
use gitprompt_core::spec::Spec;
use gitprompt_store::store::Sample;

/// Characters of a string sample kept.
pub const SAMPLE_CHARS: usize = 200;
/// In a spec's `documented`: fields every event of the source carries.
pub const ANY_EVENT: &str = "*";

/// One (source, event, path).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NotCaptured {
    pub source: String,
    pub event: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
pub fn build_catalog<'a>(samples: &[Sample], specs: impl IntoIterator<Item = &'a Spec>) -> Catalog {
    let specs: Vec<&Spec> = specs.into_iter().collect();
    let mut fields = seen(samples);
    for spec in &specs {
        mark_documented(&mut fields, spec);
        mark_unused(&mut fields, spec);
    }
    let not_captured = specs
        .iter()
        .flat_map(|spec| {
            let mut reasons = spec.not_captured.clone();
            reasons.extend(spec.drop_events.iter().map(|(event, reason)| (event.clone(), reason.clone())));
            reasons.into_iter().map(|(event, reason)| NotCaptured { source: spec.id.clone(), event, reason })
        })
        .collect();
    Catalog { fields: fields.into_values().collect(), not_captured, derived: crate::derivations::docs() }
}

/// (source, event, path): the catalog's sort order.
type Key = (String, String, String);

fn field(source: &str, event: &str, path: &str) -> Field {
    Field { source: source.into(), event: event.into(), path: path.into(), count: 0, sample: None, documented: false, unused: None }
}

/// Every (source, event, path) of the samples, with its count and last non-empty sample.
fn seen(samples: &[Sample]) -> BTreeMap<Key, Field> {
    let mut out: BTreeMap<Key, Field> = BTreeMap::new();
    for sample in samples {
        for (path, value) in flatten(&sample.data) {
            let key = (sample.source.clone(), sample.event.clone(), path);
            let found = out.entry(key).or_insert_with_key(|(source, event, path)| field(source, event, path));
            found.count += 1;
            let empty = is_no_value(&value) || value.as_object().is_some_and(serde_json::Map::is_empty);
            if !empty {
                found.sample = Some(match value {
                    Json::String(text) => Json::String(text.chars().take(SAMPLE_CHARS).collect()),
                    other => other,
                });
            }
        }
    }
    out
}

/// A documented (or ignored) name covers itself and everything nested under it.
fn covers(name: &str, path: &str) -> bool {
    path.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with('.') || rest.starts_with('['))
}

fn mark_documented(fields: &mut BTreeMap<Key, Field>, spec: &Spec) {
    for (event, names) in &spec.documented {
        for name in names {
            let mut matched = false;
            for f in fields.values_mut() {
                if f.source == spec.id && (event == ANY_EVENT || *event == f.event) && covers(name, &f.path) {
                    f.documented = true;
                    matched = true;
                }
            }
            if !matched {
                let key = (spec.id.clone(), event.clone(), name.clone());
                fields.insert(key, Field { documented: true, ..field(&spec.id, event, name) });
            }
        }
    }
}

fn mark_unused(fields: &mut BTreeMap<Key, Field>, spec: &Spec) {
    for f in fields.values_mut().filter(|f| f.source == spec.id) {
        f.unused = spec.ignored.iter().find(|(name, _)| covers(name, &f.path)).map(|(_, why)| why.clone());
    }
}
