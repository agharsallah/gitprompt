//! The one redaction pass every export goes through (ADR-0018), before any builder or format sees
//! the data. A format never redacts on its own: it only sees what this pass returns.
//!
//! Secrets are always hidden (`gitprompt_core::secrets::hide`, `[hidden <kind>]`). The other rules
//! follow [`Options`], whose defaults are the most private choice: no prompt text, project-relative
//! paths and `~` for the home folder, no account identity, no raw payloads.
//!
//! Port of `export/redact.py`. The patterns use look-behind (`HOME`, `ENCODED_HOME`); the
//! implementation needs `fancy-regex` (see the crate report) or an equivalent hand-written scan.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use gitprompt_core::Json;

/// Keys that hold text you typed, or text written from it: system excerpts, subagent briefs,
/// session titles, search words. Dropped unless [`Options::prompt_text`].
pub const TEXT_KEYS: &[&str] = &["text", "typed_text", "prompt", "excerpt", "label", "title", "snippet", "diff", "q"];
/// Stored hook or telemetry payloads, as received. Dropped while [`Options::drop_raw`].
pub const RAW_KEYS: &[&str] = &["raw", "fields", "context", "events"];
/// Telemetry resource attributes of the account: any key starting with one of these is identity.
pub const IDENTITY_PREFIXES: &[&str] = &["user.", "organization."];
/// Who you are: account ids and commit authors. Dropped while [`Options::drop_identity`].
pub const IDENTITY_KEYS: &[&str] = &["author", "email", "user_email", "user_id", "account_id", "account_uuid", "organization_id"];
/// Keys whose value is a project folder (a git root, or the working directory outside a repository).
pub const ROOT_KEYS: &[&str] = &["repo", "git_root", "git_worktree_root"];
/// What a masked email address becomes.
pub const HIDDEN_EMAIL: &str = "[hidden email]";

/// What an export includes. Every default is the most private choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// What you typed (and system text, subagent briefs, session titles); else only figures.
    pub prompt_text: bool,
    /// Project folders become project-relative paths (the folder itself its name), home folders `~`.
    pub relative_paths: bool,
    /// Account email, user and organization ids, commit authors; email addresses anywhere.
    pub drop_identity: bool,
    /// Stored hook and telemetry payloads.
    pub drop_raw: bool,
}

impl Default for Options {
    fn default() -> Self {
        todo!()
    }
}

impl Options {
    /// The option names, in declaration order (the API's query parameters).
    pub const NAMES: [&'static str; 4] = ["prompt_text", "relative_paths", "drop_identity", "drop_raw"];

    /// The options as a JSON object in declaration order (stated in a JSON export).
    pub fn as_json(&self) -> Json {
        todo!()
    }
}

/// Project folders named anywhere in a JSON value: string values under [`ROOT_KEYS`] that are
/// absolute paths, without a trailing `/`. A matched key is not searched further.
pub fn roots_in(_value: &Json) -> BTreeSet<String> {
    todo!()
}

/// `data` with what `options` leave out dropped (object keys) or masked (inside strings, object
/// keys included). Secrets are always hidden, and a clipped text ending in a cut token of 6+
/// non-space characters loses that token (`…` stays): half a key matches no pattern.
///
/// With `relative_paths`, project folders are those found in `data` ([`roots_in`]) plus `roots`;
/// the longest is applied first. Inside a folder a path becomes relative (`<root>/a.py` → `a.py`),
/// the folder alone becomes its name; `home` (when given and not `/`) becomes `~`, and so does any
/// `/Users/<name>` or `/home/<name>` not preceded by a word character, `~`, `.` or `-`; a home
/// folder written as one path component (`-Users-<name>-…`) becomes `…`.
pub fn redact(_data: &Json, _options: &Options, _home: Option<&str>, _roots: &BTreeSet<String>) -> Json {
    todo!()
}
