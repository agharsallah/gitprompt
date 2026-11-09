//! Add or remove GitPrompt's entries in a tool's JSON config file, safely. Port of
//! `sources/installer.py`.
//!
//! - Entries inside lists (hook handlers) are recognised by the receiver URL they contain
//!   (`Spec::marker`).
//! - Plain values we add (e.g. env vars) are recorded at install time ([`Record`]) and removed only
//!   if unchanged, so values the user had before, or edited since, are never touched.
//! - The file is never written if it can't be parsed; a timestamped backup is taken before each
//!   write; writes are atomic and keep the file mode.

use std::path::{Path, PathBuf};

use jiff::civil::DateTime;

use gitprompt_core::spec::Pattern;
use gitprompt_core::{Json, JsonMap, Result};

/// (key path, value) pairs a connect added. Serialized as `[[["env","X"],"v"], ...]`, the format the
/// Python daemon wrote to `<home>/installed/<id>.json`.
pub type Record = Vec<(Vec<String>, Json)>;

/// Replace `{{name}}` placeholders in every string of a JSON template (object keys untouched).
/// Unknown placeholders are left as they are.
pub fn render(_template: &Json, _values: &[(&str, &str)]) -> Json {
    todo!()
}

/// Objects merge recursively, lists get the patch items they don't already hold appended (in patch
/// order), and an existing value that is not a container (or of another type) is kept. Idempotent.
pub fn merge(_doc: &Json, _patch: &Json) -> Json {
    todo!()
}

/// The plain (non-list) values that merging `patch` adds to `doc`, depth first in patch order;
/// values already present (whatever their value) are not ours.
pub fn added(_doc: &Json, _patch: &Json) -> Record {
    todo!()
}

/// Remove list entries (objects) that hold our URL in one of their own string values, object
/// members whose string value holds it, and any container that becomes empty only because of that
/// removal. Containers the user left empty stay.
pub fn strip(_node: &Json, _ours: &Pattern) -> Json {
    todo!()
}

/// [`strip`] our list entries, then remove the plain values in `record` that are still unchanged
/// (and the objects that only held them).
pub fn uninstall(_doc: &JsonMap, _ours: &Pattern, _record: &Record) -> JsonMap {
    todo!()
}

/// (text, parsed object) of a JSON config file; `("", {})` if it doesn't exist; a blank file is `{}`.
/// Invalid JSON (comments included) is `Error::Config` ("... is not valid JSON ... GitPrompt won't
/// overwrite it"); a non-object is `Error::Config` ("... does not hold a JSON object").
pub fn read(_path: &Path) -> Result<(String, JsonMap)> {
    todo!()
}

/// Indented by 2, non-ASCII kept as is, with a trailing newline.
pub fn dump(_doc: &JsonMap) -> String {
    todo!()
}

/// A unified diff (`--- <path>`, `+++ <path>`, `@@` hunks with 3 lines of context) from `before` to
/// `after`; `""` when they are equal.
pub fn diff(_path: &Path, _before: &str, _after: &str) -> String {
    todo!()
}

/// Back up the current file (if any) to `<backups>/<file name>.<YYYYmmdd-HHMMSS of now>` (the
/// directory created with mode 0700), then replace it atomically, keeping its mode (0600 for a new
/// file). Returns the backup's path.
pub fn write(_path: &Path, _text: &str, _backups: &Path, _now: DateTime) -> Result<Option<PathBuf>> {
    todo!()
}
