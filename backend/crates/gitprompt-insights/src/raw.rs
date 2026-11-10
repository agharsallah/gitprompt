//! How derivations read raw payloads. Every read goes through [`Raw`], which can record the path of
//! each key it reads ([`Reads`]), so field coverage ([`crate::coverage::derived_reads`]) knows exactly
//! what the derivations use instead of keeping a hand-written list that drifts from the code. Without
//! a recorder a read costs what a plain JSON lookup costs.
//!
//! The recording follows the rules of the Python test helper it replaces (a dict that records the
//! keys read from it): a key is recorded when it is present and read, nested reads record the full
//! path (`tool_input.command`), list items are `[]`, and listing an object's entries records every key.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gitprompt_core::Json;
use gitprompt_store::EventRow;

use crate::text::truthy;

/// The payload paths read so far. Single-threaded by design: one coverage run records into one.
#[derive(Debug, Default)]
pub(crate) struct Reads(RefCell<BTreeSet<String>>);

impl Reads {
    fn record(&self, path: &str) {
        self.0.borrow_mut().insert(path.to_owned());
    }

    pub(crate) fn into_paths(self) -> BTreeSet<String> {
        self.0.into_inner()
    }
}

/// Where a traced value sits in its payload.
#[derive(Clone, Debug)]
struct Trace<'a> {
    reads: &'a Reads,
    path: Rc<str>,
}

impl<'a> Trace<'a> {
    fn child(&self, key: &str) -> Trace<'a> {
        let path = if self.path.is_empty() { key.into() } else { format!("{}.{key}", self.path).into() };
        Trace { reads: self.reads, path }
    }

    fn item(&self) -> Trace<'a> {
        Trace { reads: self.reads, path: format!("{}[]", self.path).into() }
    }
}

/// A payload value being read, optionally recording what is read from it.
#[derive(Clone, Debug)]
pub(crate) struct Raw<'a> {
    value: &'a Json,
    trace: Option<Trace<'a>>,
}

impl<'a> Raw<'a> {
    /// A payload whose reads are recorded into `reads` when given.
    pub(crate) fn new(value: &'a Json, reads: Option<&'a Reads>) -> Self {
        Raw { value, trace: reads.map(|reads| Trace { reads, path: "".into() }) }
    }

    pub(crate) fn json(&self) -> &'a Json {
        self.value
    }

    pub(crate) fn is_truthy(&self) -> bool {
        truthy(self.value)
    }

    /// Whether the payload is an object with `key` (not a read: nothing is recorded).
    pub(crate) fn has(&self, key: &str) -> bool {
        self.value.as_object().is_some_and(|map| map.contains_key(key))
    }

    /// The value of `key`, when the payload is an object that has it (recorded).
    pub(crate) fn get(&self, key: &str) -> Option<Raw<'a>> {
        let value = self.value.as_object()?.get(key)?;
        Some(self.child(value, key))
    }

    /// The value of `key` when it is truthy.
    pub(crate) fn truthy(&self, key: &str) -> Option<Raw<'a>> {
        self.get(key).filter(Raw::is_truthy)
    }

    /// The first of `keys` whose value is truthy.
    pub(crate) fn first(&self, keys: &[&str]) -> Option<Raw<'a>> {
        keys.iter().find_map(|key| self.truthy(key))
    }

    /// Follow a dotted path such as `message.content` or `workspace_roots[0]`; a key that itself
    /// contains dots (`app.entrypoint`) is matched as-is first (`gitprompt_core::json::pluck`).
    pub(crate) fn pluck(&self, path: &str) -> Option<Raw<'a>> {
        if self.has(path) {
            return self.get(path);
        }
        path.split(['.', '[', ']']).filter(|key| !key.is_empty()).try_fold(self.clone(), |data, key| match data.value {
            Json::Object(_) => data.get(key),
            Json::Array(items) if key.bytes().all(|b| b.is_ascii_digit()) => {
                let item = items.get(key.parse::<usize>().ok()?)?;
                Some(Raw { value: item, trace: data.trace.as_ref().map(Trace::item) })
            }
            _ => None,
        })
    }

    /// Every entry of an object payload (each key recorded); none for other values.
    pub(crate) fn entries(&self) -> Vec<(&'a str, Raw<'a>)> {
        match self.value {
            Json::Object(map) => map.iter().map(|(key, value)| (key.as_str(), self.child(value, key))).collect(),
            _ => Vec::new(),
        }
    }

    fn child(&self, value: &'a Json, key: &str) -> Raw<'a> {
        let trace = self.trace.as_ref().map(|trace| trace.child(key));
        if let Some(trace) = &trace {
            trace.reads.record(&trace.path);
        }
        Raw { value, trace }
    }
}

/// An event row and its payload as derivations read it.
#[derive(Clone, Debug)]
pub(crate) struct Event<'a> {
    pub row: &'a EventRow,
    pub raw: Raw<'a>,
}

impl<'a> Event<'a> {
    pub(crate) fn name(&self) -> Option<&'a str> {
        self.row.event.as_deref()
    }

    /// Whether the event has one of these names.
    pub(crate) fn is(&self, names: &[&str]) -> bool {
        self.name().is_some_and(|name| names.contains(&name))
    }
}

/// The rows' payloads, recorded into `reads` when given.
pub(crate) fn events<'a>(rows: &'a [EventRow], reads: Option<&'a Reads>) -> Vec<Event<'a>> {
    rows.iter().map(|row| Event { row, raw: Raw::new(&row.raw, reads) }).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_record_full_paths_of_present_keys_only() {
        let payload = json!({"a": {"b": 1}, "app.entrypoint": "cli", "l": [{"x": 1}]});
        let reads = Reads::default();
        let raw = Raw::new(&payload, Some(&reads));
        assert!(raw.pluck("a.b").is_some() && raw.pluck("app.entrypoint").is_some() && raw.pluck("l[0].x").is_some());
        assert!(raw.get("missing").is_none());
        let paths: Vec<String> = reads.into_paths().into_iter().collect();
        assert_eq!(paths, ["a", "a.b", "app.entrypoint", "l", "l[].x"]);
    }
}
