//! The one redaction pass every export goes through (ADR-0018), before any builder or format sees
//! the data. A format never redacts on its own: it only sees what this pass returns.
//!
//! Secrets are always hidden (`gitprompt_core::secrets::hide`, `[hidden <kind>]`). The other rules
//! follow [`Options`], whose defaults are the most private choice: no prompt text, project-relative
//! paths and `~` for the home folder, no account identity, no raw payloads.
//!
//! Port of `export/redact.py`, with one fix: the tool metadata objects (`connector`, `surface`)
//! keep their `label`, a tool's short name rather than text you typed (see [`TOOL_KEYS`]). The
//! patterns use look-around, so they are `fancy-regex` patterns.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::{NoExpand, Regex};
use serde::{Deserialize, Serialize};

use gitprompt_core::{Json, JsonMap};

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
/// Objects describing the tool behind a prompt (insights' facts), not anything you wrote: their
/// [`TOOL_TEXT_KEYS`] are kept whatever the options (Python dropped `connector.label`, so the
/// Tool column of a default export read `—`).
pub const TOOL_KEYS: &[&str] = &["connector", "surface"];
/// The text keys a [`TOOL_KEYS`] object keeps: a tool's short name.
pub const TOOL_TEXT_KEYS: &[&str] = &["label"];
/// What a string becomes if a masking pattern fails to run (fail closed: never leak on an error).
pub const HIDDEN_TEXT: &str = "[hidden text]";

/// A pattern of this module. Infallible: each is a fixed pattern (tested), or an escaped literal
/// path followed by a fixed look-ahead.
#[expect(clippy::expect_used, reason = "fixed patterns (tested) or an escaped literal plus a fixed look-ahead")]
fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a fixed pattern, or an escaped literal and a fixed look-ahead")
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| pattern(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+"));
/// The cut end of a clipped text: a token cut in half could be the start of a secret no pattern
/// matches. (`$` in Python also matches before a final newline.)
static CUT: LazyLock<Regex> = LazyLock::new(|| pattern(r"\S{6,}…(?=\n?\z)"));
/// Someone's home folder in an absolute path (macOS, Linux), whoever's it is.
static HOME: LazyLock<Regex> = LazyLock::new(|| pattern(r#"(?<![\w~.-])/(?:Users|home)/[^/\s"'<>|]+"#));
/// ...and a home folder written as one path component, as tools name per-project folders
/// (`~/.claude/projects/-Users-sam-code-app`): the whole component goes.
static ENCODED_HOME: LazyLock<Regex> = LazyLock::new(|| pattern(r#"(?<!\w)-(?:Users|home)-[^/\s"'<>|]+"#));

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
        Options { prompt_text: false, relative_paths: true, drop_identity: true, drop_raw: true }
    }
}

impl Options {
    /// The option names, in declaration order (the API's query parameters).
    pub const NAMES: [&'static str; 4] = ["prompt_text", "relative_paths", "drop_identity", "drop_raw"];

    /// The options as a JSON object in declaration order (stated in a JSON export).
    pub fn as_json(&self) -> Json {
        let values = [self.prompt_text, self.relative_paths, self.drop_identity, self.drop_raw];
        Json::Object(Options::NAMES.iter().zip(values).map(|(name, value)| (name.to_string(), Json::Bool(value))).collect())
    }

    /// Whether an object key (outside a tool metadata object) is left out.
    fn drops(self, key: &str) -> bool {
        (!self.prompt_text && TEXT_KEYS.contains(&key))
            || (self.drop_raw && RAW_KEYS.contains(&key))
            || (self.drop_identity && (IDENTITY_KEYS.contains(&key) || IDENTITY_PREFIXES.iter().any(|p| key.starts_with(p))))
    }
}

/// Project folders named anywhere in a JSON value: string values under [`ROOT_KEYS`] that are
/// absolute paths, without a trailing `/`. A matched key is not searched further.
pub fn roots_in(value: &Json) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    collect_roots(value, &mut found);
    found
}

fn collect_roots(value: &Json, found: &mut BTreeSet<String>) {
    match value {
        Json::Object(map) => {
            for (key, value) in map {
                match value.as_str().filter(|_| ROOT_KEYS.contains(&key.as_str())).and_then(as_root) {
                    Some(root) => {
                        found.insert(root);
                    }
                    None => collect_roots(value, found),
                }
            }
        }
        Json::Array(items) => items.iter().for_each(|item| collect_roots(item, found)),
        _ => {}
    }
}

/// `path` as a project folder: absolute, without a trailing `/`; `None` otherwise (`/` included).
pub(crate) fn as_root(path: &str) -> Option<String> {
    let root = path.trim_end_matches('/');
    (path.starts_with('/') && !root.is_empty()).then(|| root.to_string())
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
///
/// Text keys are dropped everywhere but for the [`TOOL_TEXT_KEYS`] of a [`TOOL_KEYS`] object.
pub fn redact(data: &Json, options: &Options, home: Option<&str>, roots: &BTreeSet<String>) -> Json {
    let paths = options.relative_paths.then(|| {
        let mut all = roots_in(data);
        all.extend(roots.iter().cloned());
        Paths::new(&all, home)
    });
    Pass { options, paths }.walk(data, false)
}

/// One redaction pass over a value.
struct Pass<'a> {
    options: &'a Options,
    paths: Option<Paths>,
}

impl Pass<'_> {
    /// `tool`: `value` is the object of a [`TOOL_KEYS`] key.
    fn walk(&self, value: &Json, tool: bool) -> Json {
        match value {
            Json::String(text) => Json::String(self.text(text)),
            Json::Array(items) => Json::Array(items.iter().map(|item| self.walk(item, false)).collect()),
            Json::Object(map) => Json::Object(self.object(map, tool)),
            other => other.clone(),
        }
    }

    fn object(&self, map: &JsonMap, tool: bool) -> JsonMap {
        map.iter()
            .filter(|(key, _)| !self.drops(key, tool))
            .map(|(key, value)| (self.text(key), self.walk(value, TOOL_KEYS.contains(&key.as_str()))))
            .collect()
    }

    fn drops(&self, key: &str, tool: bool) -> bool {
        !(tool && TOOL_TEXT_KEYS.contains(&key)) && self.options.drops(key)
    }

    /// A string with its secrets hidden, its cut token removed, and (per the options) its email
    /// addresses masked and its paths made relative.
    fn text(&self, value: &str) -> String {
        let mut text = replace(&CUT, &gitprompt_core::secrets::hide(value), "…");
        if self.options.drop_identity {
            text = replace(&EMAIL, &text, HIDDEN_EMAIL);
        }
        match &self.paths {
            Some(paths) => paths.apply(text),
            None => text,
        }
    }
}

/// Every match of `pattern` in `text` replaced by `with`, taken literally. If the pattern can't run
/// (fancy-regex's backtracking limit), the whole text is hidden instead: fail closed.
fn replace(pattern: &Regex, text: &str, with: &str) -> String {
    match pattern.try_replacen(text, 0, NoExpand(with)) {
        Ok(replaced) => replaced.into_owned(),
        Err(error) => {
            tracing::warn!(%error, "a redaction pattern failed; the text is hidden");
            HIDDEN_TEXT.to_string()
        }
    }
}

/// Absolute paths made relative: inside a project folder, relative to it (the folder itself by its
/// name); in a home folder, from `~`.
struct Paths {
    rules: Vec<(Regex, String)>,
}

impl Paths {
    fn new(roots: &BTreeSet<String>, home: Option<&str>) -> Paths {
        let mut sorted: Vec<&String> = roots.iter().filter(|root| !root.is_empty() && root.as_str() != "/").collect();
        // Longest first, so a nested folder wins over its parent; ties in name order (deterministic).
        sorted.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        let mut rules = Vec::with_capacity(sorted.len() * 2 + 1);
        for root in sorted {
            let escaped = fancy_regex::escape(root);
            let name = Path::new(root.as_str()).file_name().and_then(|n| n.to_str()).unwrap_or(root).to_string();
            rules.push((pattern(&format!(r"{escaped}/(?=\S)")), String::new()));
            rules.push((pattern(&format!(r"{escaped}(?![\w.-])")), name));
        }
        if let Some(home) = home.map(|h| h.trim_end_matches('/')).filter(|h| !h.is_empty()) {
            rules.push((pattern(&format!(r"{}(?![\w.-])", fancy_regex::escape(home))), "~".to_string()));
        }
        Paths { rules }
    }

    fn apply(&self, text: String) -> String {
        let text = self.rules.iter().fold(text, |text, (pattern, with)| replace(pattern, &text, with));
        replace(&ENCODED_HOME, &replace(&HOME, &text, "~"), "…")
    }
}
