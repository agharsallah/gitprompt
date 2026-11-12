//! Pure helpers over JSON payloads (objects, arrays, scalars). No I/O, no knowledge of any tool.
//! Port of `core/payload.py`.

use indexmap::IndexMap;

use crate::blank::Blank;

pub type Json = serde_json::Value;
pub type JsonMap = serde_json::Map<String, Json>;

/// field -> candidate paths; the first candidate holding a value wins.
pub type Mapping = IndexMap<String, Vec<String>>;

/// Default length above which [`clip`] shortens strings.
pub const CLIP_LIMIT: usize = 10_000;

/// Follow a dotted path such as `message.content` or `workspace_roots[0]`. A key that itself contains
/// dots (OpenTelemetry's `prompt.id`) is matched as-is first. Missing or mistyped steps give `None`.
pub fn pluck<'a>(data: &'a Json, path: &str) -> Option<&'a Json> {
    if let Some(value) = data.as_object().and_then(|map| map.get(path)) {
        return Some(value);
    }
    path.split(['.', '[', ']']).filter(|key| !key.is_empty()).try_fold(data, step)
}

/// One step of a path: an object key, or a list index written in digits.
fn step<'a>(data: &'a Json, key: &str) -> Option<&'a Json> {
    match data {
        Json::Object(map) => map.get(key),
        Json::Array(items) if key.bytes().all(|b| b.is_ascii_digit()) => items.get(key.parse::<usize>().ok()?),
        _ => None,
    }
}

/// Whether a mapped value counts as "no value" and falls through to the next candidate:
/// null, `""` and `[]` (but not `{}`, `0` or `false`).
pub fn is_no_value(value: &Json) -> bool {
    match value {
        Json::Null => true,
        Json::String(s) => s.is_empty(),
        Json::Array(items) => items.is_empty(),
        _ => false,
    }
}

/// The first value among candidate paths that is not "no value".
pub fn first<'a>(data: &'a Json, paths: &[String]) -> Option<&'a Json> {
    paths.iter().filter_map(|path| pluck(data, path)).find(|value| !is_no_value(value))
}

/// Apply a spec's field mapping to a payload. Fields with no value are left out; field order follows
/// the mapping.
pub fn extract(payload: &Json, mapping: &Mapping) -> JsonMap {
    mapping.iter().filter_map(|(field, paths)| Some((field.clone(), first(payload, paths)?.clone()))).collect()
}

/// Apply `f` to every string in a JSON value, keeping its structure (object keys are untouched).
pub fn map_strings(value: &Json, f: &dyn Fn(&str) -> String) -> Json {
    match value {
        Json::String(s) => Json::String(f(s)),
        Json::Array(items) => Json::Array(items.iter().map(|v| map_strings(v, f)).collect()),
        Json::Object(map) => Json::Object(map.iter().map(|(k, v)| (k.clone(), map_strings(v, f))).collect()),
        other => other.clone(),
    }
}

/// Shorten strings longer than `limit` characters (not bytes) to
/// `"<first limit chars>… [<n> more characters not stored]"`.
pub fn clip(value: &Json, limit: usize) -> Json {
    map_strings(value, &|text| shorten(text, limit))
}

fn shorten(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        None => text.to_owned(),
        Some((cut, _)) => {
            let rest = text[cut..].chars().count();
            format!("{}… [{rest} more characters not stored]", &text[..cut])
        }
    }
}

/// (path, leaf) pairs. Object keys join with `.`, list items share one path ending in `[]`;
/// an empty object or list is a leaf itself.
pub fn flatten(value: &Json) -> Vec<(String, Json)> {
    let mut out = Vec::new();
    flatten_into(value, String::new(), &mut out);
    out
}

fn flatten_into(value: &Json, path: String, out: &mut Vec<(String, Json)>) {
    match value {
        Json::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                let child_path = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                flatten_into(child, child_path, out);
            }
        }
        Json::Array(items) if !items.is_empty() => {
            for child in items {
                flatten_into(child, format!("{path}[]"), out);
            }
        }
        leaf => out.push((path, leaf.clone())),
    }
}

/// Drop null, empty (`""`, `[]`, `{}`), `false` and zero values, so summaries show what happened.
pub fn compact(data: &JsonMap) -> JsonMap {
    data.iter().filter(|(_, v)| !v.is_blank()).map(|(k, v)| (k.clone(), v.clone())).collect()
}
