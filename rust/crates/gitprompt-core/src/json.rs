//! Pure helpers over JSON payloads (objects, arrays, scalars). No I/O, no knowledge of any tool.
//! Port of `core/payload.py`.

use indexmap::IndexMap;

pub type Json = serde_json::Value;
pub type JsonMap = serde_json::Map<String, Json>;

/// field -> candidate paths; the first candidate holding a value wins.
pub type Mapping = IndexMap<String, Vec<String>>;

/// Default length above which [`clip`] shortens strings.
pub const CLIP_LIMIT: usize = 10_000;

/// Follow a dotted path such as `message.content` or `workspace_roots[0]`. A key that itself contains
/// dots (OpenTelemetry's `prompt.id`) is matched as-is first. Missing or mistyped steps give `None`.
pub fn pluck<'a>(_data: &'a Json, _path: &str) -> Option<&'a Json> {
    todo!()
}

/// Whether a mapped value counts as "no value" and falls through to the next candidate:
/// null, `""` and `[]` (but not `{}`, `0` or `false`).
pub fn is_no_value(_value: &Json) -> bool {
    todo!()
}

/// The first value among candidate paths that is not "no value".
pub fn first<'a>(_data: &'a Json, _paths: &[String]) -> Option<&'a Json> {
    todo!()
}

/// Apply a spec's field mapping to a payload. Fields with no value are left out; field order follows
/// the mapping.
pub fn extract(_payload: &Json, _mapping: &Mapping) -> JsonMap {
    todo!()
}

/// Apply `f` to every string in a JSON value, keeping its structure (object keys are untouched).
pub fn map_strings(_value: &Json, _f: &dyn Fn(&str) -> String) -> Json {
    todo!()
}

/// Shorten strings longer than `limit` characters (not bytes) to
/// `"<first limit chars>… [<n> more characters not stored]"`.
pub fn clip(_value: &Json, _limit: usize) -> Json {
    todo!()
}

/// (path, leaf) pairs. Object keys join with `.`, list items share one path ending in `[]`;
/// an empty object or list is a leaf itself.
pub fn flatten(_value: &Json) -> Vec<(String, Json)> {
    todo!()
}

/// Drop null, empty (`""`, `[]`, `{}`), `false` and zero values, so summaries show what happened.
pub fn compact(_data: &JsonMap) -> JsonMap {
    todo!()
}
