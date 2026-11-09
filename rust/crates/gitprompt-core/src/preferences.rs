//! User preferences: what the Settings page can change, stored in `<home>/preferences.json`.
//!
//! Each value comes from the first of: `GITPROMPT_<KEY>` env var (locked: read-only on the page),
//! the file (written atomically, mode 0600), the default. The file and env are re-read on every use,
//! so changes apply without a restart. Port of `core/preferences.py`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Env, Json, JsonMap, Result};

pub const ENV_PREFIX: &str = "GITPROMPT_";
pub const FILE_NAME: &str = "preferences.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeId {
    #[serde(rename = "24h")]
    Day,
    #[default]
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeekStart {
    #[default]
    Monday,
    Sunday,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Preferences {
    /// Time window the insight pages open with.
    pub default_range: RangeId,
    /// First row of the weekday heatmap.
    pub week_start: WeekStart,
    /// Raw events older than this are deleted (1..=3650); `None` keeps them forever.
    pub event_retention_days: Option<u32>,
    /// Replace detected secrets before a payload is stored.
    pub hide_secrets_on_ingest: bool,
    /// Connectors turned off in GitPrompt: what they send is discarded. Env: comma-separated ids.
    pub disabled_sources: Vec<String>,
}

/// Every preference key, in declaration order.
pub const KEYS: &[&str] = &["default_range", "week_start", "event_retention_days", "hide_secrets_on_ingest", "disabled_sources"];

/// Keys that need a daemon restart to take effect (none today).
pub const APPLIES_AT_START: &[&str] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueSource {
    Env,
    File,
    Default,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Applies {
    Now,
    Restart,
}

/// One preference with its effective value and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resolved {
    pub key: String,
    pub value: Json,
    pub default: Json,
    pub source: ValueSource,
    /// The variable that would lock it, e.g. `GITPROMPT_WEEK_START`.
    pub env: String,
    pub applies: Applies,
    pub locked: bool,
}

/// `GITPROMPT_` + the key in upper case.
pub fn env_name(_key: &str) -> String {
    todo!()
}

/// `preferences.json` under a data directory, plus the env vars that override it.
pub struct PreferenceFile {
    pub path: PathBuf,
    pub env: Env,
}

impl PreferenceFile {
    pub fn new(home: &Path, env: Env) -> Self {
        PreferenceFile { path: home.join(FILE_NAME), env }
    }

    /// The values the file sets (validated). Missing file = `{}`; an empty file = `{}`. Invalid JSON,
    /// a non-object or an invalid value = `Error::InvalidPreferences` naming the file.
    pub fn stored(&self) -> Result<JsonMap> {
        todo!()
    }

    /// Preferences set by a non-empty (after trimming) `GITPROMPT_<KEY>` variable, as raw strings.
    pub fn overrides(&self) -> JsonMap {
        todo!()
    }

    /// Every key (in [`KEYS`] order) with its effective value and source. An invalid env value is an
    /// `Error::InvalidPreferences` mentioning "environment variable".
    pub fn resolve(&self) -> Result<Vec<Resolved>> {
        todo!()
    }

    pub fn current(&self) -> Result<Preferences> {
        todo!()
    }

    /// [`Self::current`], or the defaults when the file or an env var is invalid: capture and
    /// retention keep working on safe values while the Settings page reports the error.
    pub fn effective(&self) -> Preferences {
        todo!()
    }

    /// Set values in the file; `null` removes a key (back to the default). Returns the keys whose
    /// effective value changed. Nothing is written if any change is refused: unknown key
    /// (`InvalidPreferences`), locked by env (`Locked`, naming the variable), invalid value
    /// (`InvalidPreferences`). The file is written sorted, indented, atomically, mode 0600.
    pub fn update(&self, _changes: &JsonMap) -> Result<Vec<String>> {
        todo!()
    }
}
