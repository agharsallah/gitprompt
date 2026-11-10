//! Connecting a source = merging its spec's `install.merge` into the tool's config file, with
//! consent: every change can be previewed (dry run) as the exact diff before it is written.
//! This module holds the workflow (port of `sources/connection.py` plus the capture-side parts of
//! `api/routes/sources.py`: enable/disable, remap); [`crate::installer`] holds the JSON-file
//! mechanics. It also computes a connector's state ([`state`]; README "Connectors", formerly
//! computed in the dashboard's `state.ts`).
//!
//! ## How hooks deliver
//!
//! The bundled specs install curl commands that pipe the hook payload to the receiver; they work
//! with nothing but curl, and lose the payload when GitPrompt isn't running. A connection made
//! [`Connection::with_forwarder`] installs GitPrompt's own forwarder instead (the desktop app),
//! which queues payloads while the daemon is down ([`crate::spool`]): every hook command of the spec
//! (a string starting with `curl ` that posts to `{{receiver}}/ingest/{{source}}`) becomes
//!
//! ```text
//! '<forwarder path>' hook <source> # /ingest/<source>
//! ```
//!
//! The trailing shell comment is the marker ([`Spec::marker`]) by which `connected`, `outdated`,
//! `disconnect` and token rotation recognise GitPrompt's entries, in either form; the hook commands
//! of every bundled spec already run through a shell. The forwarder reads the receiver and token
//! itself, so these commands carry no token, and prints the reply the tool expects
//! (`install.reply`). Other strings (e.g. the telemetry endpoint) are rendered as usual.
//!
//! Use one form consistently: a source connected in one form is [`Connection::outdated`] when
//! checked in the other, and reconnecting switches it.

mod hooks;
pub mod state;

use std::path::{Path, PathBuf};

use jiff::Zoned;
use jiff::civil::DateTime;
use serde::{Deserialize, Serialize};

use gitprompt_core::files::atomic_write;
use gitprompt_core::preferences::{PreferenceFile, Preferences};
use gitprompt_core::spec::{Join, Spec, expand};
use gitprompt_core::{Error, Json, JsonMap, Result, Settings};
use gitprompt_store::Store;

use crate::installer::{self, Record, create_private_dir};
use crate::probe::check_policy;

pub use hooks::HookForm;
pub use state::{Action, ConnectorState, QUIET_AFTER, StateFacts};

/// One config-file change, previewed or applied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub file: String,
    /// Unified diff; empty when there is nothing to change.
    pub diff: String,
    pub applied: bool,
    /// The backup taken before writing, when a file existed.
    pub backup: Option<String>,
}

/// GitPrompt's entries for one source in one tool config file.
pub struct Connection<'a> {
    pub spec: &'a Spec,
    pub settings: &'a Settings,
    /// Render with this token instead of the current one (token rotation preview).
    pub token: Option<String>,
    /// Time used to name backups; `None` = the local time when writing.
    pub now: Option<DateTime>,
    /// Install hook commands that run this forwarder executable instead of curl (see the module
    /// docs); `None` = the spec's curl commands.
    pub forwarder: Option<PathBuf>,
}

impl<'a> Connection<'a> {
    pub fn new(spec: &'a Spec, settings: &'a Settings) -> Self {
        Connection { spec, settings, token: None, now: None, forwarder: None }
    }

    /// The same connection rendered with `token` (e.g. [`crate::rotation::PLACEHOLDER`]).
    pub fn with_token(self, token: impl Into<String>) -> Self {
        Connection { token: Some(token.into()), ..self }
    }

    /// The same connection naming backups after `now`.
    pub fn at(self, now: DateTime) -> Self {
        Connection { now: Some(now), ..self }
    }

    /// The same connection installing hooks that run GitPrompt's forwarder at `path` (see the
    /// module docs).
    pub fn with_forwarder(self, path: impl Into<PathBuf>) -> Self {
        Connection { forwarder: Some(path.into()), ..self }
    }

    /// The same connection with `forwarder` set or cleared.
    pub fn via(self, forwarder: Option<&Path>) -> Self {
        Connection { forwarder: forwarder.map(Path::to_path_buf), ..self }
    }

    /// `install.file`, expanded (`${VAR:-default}` from `settings.env`, `~` = `settings.user_home`).
    pub fn config_file(&self) -> PathBuf {
        expand(&self.spec.install.file, self.settings)
    }

    /// `<home>/installed/<id>.json`: the plain values added at connect time, so disconnect removes
    /// exactly those.
    pub fn record_file(&self) -> PathBuf {
        self.settings.home.join("installed").join(format!("{}.json", self.spec.id))
    }

    /// `install.merge` with `{{receiver}}`, `{{source}}` (the spec id) and `{{token}}` (the override,
    /// else `settings.token()`) filled in; with a forwarder, its hook commands replaced first.
    /// `Error::Invalid` when a forwarder is set and the spec id or the forwarder path can't be put
    /// in a shell command safely.
    pub fn patch(&self) -> Result<Json> {
        let mut template = Json::Object(self.spec.install.merge.clone());
        if let Some(forwarder) = &self.forwarder {
            template = hooks::replace_curl_commands(&template, &hooks::forwarder_command(forwarder, &self.spec.id)?);
        }
        let token = match &self.token {
            Some(token) => token.clone(),
            None => self.settings.token()?,
        };
        let receiver = self.settings.receiver();
        Ok(installer::render(&template, &[("receiver", &receiver), ("source", &self.spec.id), ("token", &token)]))
    }

    /// The config file exists and holds this source's receiver URL (`Spec::marker`).
    pub fn connected(&self) -> bool {
        std::fs::read_to_string(self.config_file()).is_ok_and(|text| self.spec.marker().is_match(&text))
    }

    /// How the installed hook commands deliver: curl, or GitPrompt's forwarder (and which
    /// executable). `None` when not connected.
    pub fn installed_form(&self) -> Option<HookForm> {
        let text = std::fs::read_to_string(self.config_file()).ok()?;
        self.spec.marker().is_match(&text).then(|| hooks::installed_form(&text, &self.spec.id))
    }

    /// Connected, but the spec now adds more (or other) entries than what is installed. `false`
    /// when not connected or the file can't be parsed.
    pub fn outdated(&self) -> bool {
        if !self.connected() {
            return false;
        }
        match self.patch().and_then(|patch| self.change(Some(&patch), true)) {
            Ok(change) => !change.diff.is_empty(),
            Err(e) => {
                tracing::debug!(source = %self.spec.id, error = %e, "can't tell whether the connection is outdated");
                false
            }
        }
    }

    /// Remove our previous entries, then merge the patch: reconnecting never duplicates. Refused
    /// with `Error::PolicyBlocked` ("Blocked by settings GitPrompt won't override: <blocks>") when
    /// policy blocks it, before anything is read or written; `Error::Config` for a config file that
    /// can't be parsed (left untouched). A dry run writes nothing. Applying writes the file (backup
    /// first) and the record of the plain values added.
    pub fn connect(&self, dry_run: bool) -> Result<Change> {
        let blocks = check_policy(self.spec, self.settings).blocks;
        if !blocks.is_empty() {
            return Err(Error::PolicyBlocked(format!("Blocked by settings GitPrompt won't override: {}", blocks.join("; "))));
        }
        self.change(Some(&self.patch()?), dry_run)
    }

    /// Remove our list entries and the recorded plain values the user hasn't changed since. Nothing
    /// to remove = an empty diff, not applied, no write.
    pub fn disconnect(&self, dry_run: bool) -> Result<Change> {
        self.change(None, dry_run)
    }

    /// Remove our previous entries, then add `patch` (if any).
    fn change(&self, patch: Option<&Json>, dry_run: bool) -> Result<Change> {
        let path = self.config_file();
        let file = path.display().to_string();
        let (before, doc) = installer::read(&path)?;
        let clean = installer::uninstall(&doc, &self.spec.marker(), &self.read_record());
        let clean = Json::Object(clean);
        let new = match patch {
            Some(patch) => installer::merge(&clean, patch),
            None => clean.clone(),
        };
        let Json::Object(new) = new else {
            // `merge` keeps the type of its first argument, an object here.
            return Err(Error::Config(format!("{file}: merging GitPrompt's entries did not give an object")));
        };
        if new == doc {
            return Ok(Change { file, diff: String::new(), applied: false, backup: None });
        }
        let after = installer::dump(&new);
        let diff = installer::diff(&path, &before, &after);
        if dry_run {
            return Ok(Change { file, diff, applied: false, backup: None });
        }
        let now = self.now.unwrap_or_else(|| Zoned::now().datetime());
        let backup = installer::write(&path, &after, &self.settings.backups(), now)?;
        self.write_record(&patch.map(|patch| installer::added(&clean, patch)).unwrap_or_default())?;
        Ok(Change { file, diff, applied: true, backup: backup.map(|b| b.display().to_string()) })
    }

    /// The values recorded at the last connect; none when there is no record. A record that can't be
    /// read is treated as empty (logged): the values it named then stay in the tool's file, which is
    /// the safe direction.
    fn read_record(&self) -> Record {
        let path = self.record_file();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Record::new(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "install record unreadable; treated as empty");
                return Record::new();
            }
        };
        serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(path = %path.display(), error = %e, "install record invalid; treated as empty");
            Record::new()
        })
    }

    fn write_record(&self, record: &Record) -> Result<()> {
        let path = self.record_file();
        if let Some(dir) = path.parent() {
            create_private_dir(dir)?;
        }
        atomic_write(&path, &serde_json::to_string(record)?, 0o600)?;
        Ok(())
    }
}

// --- enable, disable, remap (capture-side halves of the sources routes) ----------------------

/// `Error::Conflict` ("<name> is turned off in GitPrompt: enable it first") when the source is in
/// `disabled_sources`: a turned-off connector can't be connected.
pub fn refuse_if_disabled(spec: &Spec, preferences: &Preferences) -> Result<()> {
    if preferences.disabled_sources.contains(&spec.id) {
        return Err(Error::Conflict(format!("{} is turned off in GitPrompt: enable it first", spec.name)));
    }
    Ok(())
}

/// Turn a connector on or off in GitPrompt (preference `disabled_sources`); its entries stay in the
/// tool's config file. Other disabled sources are kept; an empty list removes the key. Returns the new
/// `enabled`. `Error::Locked` when `GITPROMPT_DISABLED_SOURCES` is set.
pub fn set_enabled(spec: &Spec, preferences: &PreferenceFile, enabled: bool) -> Result<bool> {
    let mut disabled: Vec<String> = preferences.current()?.disabled_sources.into_iter().filter(|s| *s != spec.id).collect();
    if !enabled {
        disabled.push(spec.id.clone());
    }
    let value = if disabled.is_empty() { Json::Null } else { Json::from(disabled) };
    preferences.update(&JsonMap::from_iter([("disabled_sources".to_owned(), value)]))?;
    Ok(enabled)
}

/// Rebuild mapped fields from stored raw payloads (after a spec fix); for a `join: session` source
/// the prompt ids given at ingest are not in raw, so the session join is redone. Returns rows
/// remapped. (The server rebuilds the prompt index afterwards.)
pub fn remap(spec: &Spec, store: &Store) -> Result<u64> {
    store.transaction(|store| {
        let updated = store.remap(&spec.id, &|raw| spec.extract(raw))?;
        if spec.join == Join::Session {
            store.rejoin(&spec.id)?;
        }
        Ok(updated)
    })
}
