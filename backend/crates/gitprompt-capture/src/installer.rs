//! Add or remove GitPrompt's entries in a tool's JSON config file, safely. Port of
//! `sources/installer.py`.
//!
//! - Entries inside lists (hook handlers) are recognised by the receiver URL they contain
//!   (`Spec::marker`).
//! - Plain values we add (e.g. env vars) are recorded at install time ([`Record`]) and removed only
//!   if unchanged, so values the user had before, or edited since, are never touched.
//! - The file is never written if it can't be parsed; a timestamped backup is taken before each
//!   write; writes are atomic and keep the file mode.

use std::io;
use std::path::{Path, PathBuf};

use jiff::civil::DateTime;

use gitprompt_core::files::atomic_write;
use gitprompt_core::spec::Pattern;
use gitprompt_core::{Error, Json, JsonMap, Result};

/// (key path, value) pairs a connect added. Serialized as `[[["env","X"],"v"], ...]`, the format the
/// Python daemon wrote to `<home>/installed/<id>.json`.
pub type Record = Vec<(Vec<String>, Json)>;

/// Mode of a config file GitPrompt creates.
const NEW_FILE_MODE: u32 = 0o600;
/// Mode of the backups directory (backups hold the user's settings, tokens included).
const BACKUPS_MODE: u32 = 0o700;

/// Replace `{{name}}` placeholders in every string of a JSON template (object keys untouched).
/// Unknown placeholders are left as they are.
pub fn render(template: &Json, values: &[(&str, &str)]) -> Json {
    gitprompt_core::json::map_strings(template, &|text| {
        values.iter().fold(text.to_owned(), |text, (name, value)| text.replace(&format!("{{{{{name}}}}}"), value))
    })
}

/// Objects merge recursively, lists get the patch items they don't already hold appended (in patch
/// order), and an existing value that is not a container (or of another type) is kept. Idempotent.
pub fn merge(doc: &Json, patch: &Json) -> Json {
    match (doc, patch) {
        (Json::Object(doc), Json::Object(patch)) => {
            let mut out = doc.clone();
            for (key, value) in patch {
                let merged = match doc.get(key) {
                    Some(existing) => merge(existing, value),
                    None => value.clone(),
                };
                // An existing key keeps its position: `insert` replaces the value in place.
                out.insert(key.clone(), merged);
            }
            Json::Object(out)
        }
        (Json::Array(doc), Json::Array(patch)) => {
            let missing = patch.iter().filter(|item| !doc.contains(item)).cloned();
            Json::Array(doc.iter().cloned().chain(missing).collect())
        }
        _ => doc.clone(),
    }
}

/// The plain (non-list) values that merging `patch` adds to `doc`, depth first in patch order;
/// values already present (whatever their value) are not ours.
pub fn added(doc: &Json, patch: &Json) -> Record {
    let mut out = Record::new();
    added_into(Some(doc), patch, &mut Vec::new(), &mut out);
    out
}

fn added_into(doc: Option<&Json>, patch: &Json, path: &mut Vec<String>, out: &mut Record) {
    match (doc, patch) {
        (None | Some(Json::Object(_)), Json::Object(patch)) => {
            for (key, value) in patch {
                let child = doc.and_then(|d| d.get(key));
                path.push(key.clone());
                added_into(child, value, path, out);
                path.pop();
            }
        }
        // Lists are recognised by our URL (`strip`); anything already there is the user's, and an
        // existing non-object under an object patch is kept by `merge`, so nothing is added.
        (None, Json::Array(_)) | (Some(_), _) => {}
        (None, value) => out.push((path.clone(), value.clone())),
    }
}

/// Remove list entries (objects) that hold our URL in one of their own string values, object
/// members whose string value holds it, and any container that becomes empty only because of that
/// removal. Containers the user left empty stay.
pub fn strip(node: &Json, ours: &Pattern) -> Json {
    match node {
        Json::Array(items) => {
            Json::Array(items.iter().filter(|item| !holds_ours(item, ours)).filter_map(|item| kept(item, strip(item, ours))).collect())
        }
        Json::Object(map) => Json::Object(strip_members(map, ours)),
        other => other.clone(),
    }
}

fn strip_members(map: &JsonMap, ours: &Pattern) -> JsonMap {
    map.iter()
        .filter(|(_, value)| !is_ours(value, ours))
        .filter_map(|(key, value)| kept(value, strip(value, ours)).map(|new| (key.clone(), new)))
        .collect()
}

/// `new` (the stripped `old`), unless stripping emptied a container that wasn't empty before.
fn kept(old: &Json, new: Json) -> Option<Json> {
    (!is_empty_container(&new) || &new == old).then_some(new)
}

/// A list entry of ours: an object with our URL in one of its own string values.
fn holds_ours(item: &Json, ours: &Pattern) -> bool {
    item.as_object().is_some_and(|map| map.values().any(|value| is_ours(value, ours)))
}

fn is_ours(value: &Json, ours: &Pattern) -> bool {
    value.as_str().is_some_and(|text| ours.is_match(text))
}

fn is_empty_container(value: &Json) -> bool {
    match value {
        Json::Object(map) => map.is_empty(),
        Json::Array(items) => items.is_empty(),
        _ => false,
    }
}

/// [`strip`] our list entries, then remove the plain values in `record` that are still unchanged
/// (and the objects that only held them).
pub fn uninstall(doc: &JsonMap, ours: &Pattern, record: &Record) -> JsonMap {
    let mut doc = strip_members(doc, ours);
    for (path, value) in record {
        remove_unchanged(&mut doc, path, value);
    }
    doc
}

/// Remove the value at `path` if it still equals `value`, then the objects on the way that this
/// left empty. Returns whether `map` itself is now empty because of it.
fn remove_unchanged(map: &mut JsonMap, path: &[String], value: &Json) -> bool {
    let Some((key, rest)) = path.split_first() else { return false };
    if rest.is_empty() {
        if map.get(key) != Some(value) {
            return false; // changed by the user since (or gone): keep it
        }
        map.shift_remove(key);
        return map.is_empty();
    }
    let Some(Json::Object(child)) = map.get_mut(key) else { return false };
    if remove_unchanged(child, rest, value) {
        map.shift_remove(key);
        return map.is_empty();
    }
    false
}

/// (text, parsed object) of a JSON config file; `("", {})` if it doesn't exist; a blank file is `{}`.
/// Invalid JSON (comments included) is `Error::Config` ("... is not valid JSON ... GitPrompt won't
/// overwrite it"); a non-object is `Error::Config` ("... does not hold a JSON object").
pub fn read(path: &Path) -> Result<(String, JsonMap)> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((String::new(), JsonMap::new())),
        Err(e) => return Err(Error::Config(format!("{} can't be read ({e}); GitPrompt won't overwrite it", path.display()))),
    };
    if text.trim().is_empty() {
        return Ok((text, JsonMap::new()));
    }
    let doc: Json = serde_json::from_str(&text)
        .map_err(|e| Error::Config(format!("{} is not valid JSON ({e}); fix it first, GitPrompt won't overwrite it", path.display())))?;
    match doc {
        Json::Object(map) => Ok((text, map)),
        _ => Err(Error::Config(format!("{} does not hold a JSON object", path.display()))),
    }
}

/// Indented by 2, non-ASCII kept as is, with a trailing newline.
pub fn dump(doc: &JsonMap) -> String {
    let mut text = serde_json::to_string_pretty(doc).unwrap_or_else(|e| {
        // A map of `Json` values always serializes (string keys, finite numbers): unreachable.
        tracing::error!(error = %e, "could not serialize a JSON document");
        String::from("{}")
    });
    text.push('\n');
    text
}

/// A unified diff (`--- <path>`, `+++ <path>`, `@@` hunks with 3 lines of context) from `before` to
/// `after`; `""` when they are equal.
pub fn diff(path: &Path, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let name = path.display().to_string();
    similar::TextDiff::from_lines(before, after).unified_diff().context_radius(3).header(&name, &name).to_string()
}

/// Back up the current file (if any) to `<backups>/<file name>.<YYYYmmdd-HHMMSS of now>` (the
/// directory created with mode 0700), then replace it atomically, keeping its mode (0600 for a new
/// file). Returns the backup's path.
///
/// A backup never replaces an earlier one: when that name is taken (two writes within a second), a
/// `-2`, `-3`... suffix is added, so the copy taken before GitPrompt's first change is never lost.
pub fn write(path: &Path, text: &str, backups: &Path, now: DateTime) -> Result<Option<PathBuf>> {
    let existing = match std::fs::metadata(path) {
        Ok(meta) => Some(meta),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let backup = match &existing {
        Some(_) => Some(back_up(path, backups, now)?),
        None => None,
    };
    let mode = existing.as_ref().map_or(NEW_FILE_MODE, file_mode);
    atomic_write(path, text, mode)?;
    Ok(backup)
}

/// Copy `path` into `backups` under a name no earlier backup has.
fn back_up(path: &Path, backups: &Path, now: DateTime) -> Result<PathBuf> {
    create_private_dir(backups)?;
    let name = path.file_name().map_or_else(|| "config".into(), |n| n.to_string_lossy().into_owned());
    let stamp = now.strftime("%Y%m%d-%H%M%S").to_string();
    for n in 1u32.. {
        let candidate = match n {
            1 => backups.join(format!("{name}.{stamp}")),
            n => backups.join(format!("{name}.{stamp}-{n}")),
        };
        // `create_new` claims the name atomically, so two writers never pick the same one.
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => {
                std::fs::copy(path, &candidate)?;
                return Ok(candidate);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Conflict(format!("no free backup name for {}", path.display())))
}

/// Create `dir` (and its parents) with mode 0700; an existing directory keeps its mode.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, BACKUPS_MODE);
    builder.create(dir)
}

#[cfg(unix)]
fn file_mode(meta: &std::fs::Metadata) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777
}

#[cfg(not(unix))]
fn file_mode(_meta: &std::fs::Metadata) -> u32 {
    NEW_FILE_MODE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn marker() -> Pattern {
        Pattern::new(r"/ingest/x(?![\w-])").unwrap()
    }

    #[test]
    fn added_ignores_children_of_an_existing_non_object() {
        assert_eq!(added(&json!({"env": "set by the user"}), &json!({"env": {"A": "1"}})), Record::new());
    }

    #[test]
    fn a_container_the_user_left_empty_survives_strip() {
        let doc = json!({"hooks": {"Stop": []}, "env": {}});
        assert_eq!(strip(&doc, &marker()), doc);
    }

    #[test]
    fn uninstall_keeps_a_value_whose_parent_is_no_longer_an_object() {
        let doc = gitprompt_core::JsonMap::from_iter([("env".to_string(), json!("x"))]);
        let record = vec![(vec!["env".to_string(), "A".to_string()], json!("1"))];
        assert_eq!(uninstall(&doc, &marker(), &record), doc);
    }

    #[test]
    fn a_second_backup_within_the_same_second_gets_its_own_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let backups = dir.path().join("backups");
        let now = jiff::civil::date(2026, 11, 7).at(10, 11, 12, 0);
        std::fs::write(&path, "first").unwrap();
        let first = write(&path, "second", &backups, now).unwrap().unwrap();
        let second = write(&path, "third", &backups, now).unwrap().unwrap();
        assert_eq!(second, backups.join("settings.json.20261107-101112-2"));
        assert_eq!(std::fs::read_to_string(first).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "second");
    }
}
