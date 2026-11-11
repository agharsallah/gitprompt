//! User preferences: what the Settings page can change, stored in `<home>/preferences.json`.
//!
//! Each value comes from the first of: `GITPROMPT_<KEY>` env var (locked: read-only on the page),
//! the file (written atomically, mode 0600), the default. The file and env are re-read on every use,
//! so changes apply without a restart. Port of `core/preferences.py`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files::atomic_write;
use crate::{Env, Error, Json, JsonMap, Result};

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
pub fn env_name(key: &str) -> String {
    format!("{ENV_PREFIX}{}", key.to_uppercase())
}

/// `preferences.json` under a data directory, plus the env vars that override it.
pub struct PreferenceFile {
    pub path: PathBuf,
    pub env: Env,
}

impl std::fmt::Debug for PreferenceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreferenceFile").field("path", &self.path).finish_non_exhaustive()
    }
}

impl PreferenceFile {
    pub fn new(home: &Path, env: Env) -> Self {
        PreferenceFile { path: home.join(FILE_NAME), env }
    }

    /// The values the file sets (validated). Missing file = `{}`; an empty file = `{}`. Invalid JSON,
    /// a non-object or an invalid value = `Error::InvalidPreferences` naming the file.
    pub fn stored(&self) -> Result<JsonMap> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(JsonMap::new()),
            Err(e) => return Err(e.into()),
        };
        if text.is_empty() {
            return Ok(JsonMap::new());
        }
        let path = self.path.display();
        let data: Json = serde_json::from_str(&text)
            .map_err(|e| Error::InvalidPreferences(format!("{path} is not valid JSON ({e}); fix or delete it")))?;
        let Json::Object(data) = data else {
            return Err(Error::InvalidPreferences(format!("{path} must hold a JSON object; fix or delete it")));
        };
        normalize_all(&data, &path.to_string())?;
        Ok(data)
    }

    /// Preferences set by a non-empty (after trimming) `GITPROMPT_<KEY>` variable, as raw strings.
    pub fn overrides(&self) -> JsonMap {
        KEYS.iter()
            .filter_map(|&key| {
                let value = self.env.get(&env_name(key))?.trim();
                (!value.is_empty()).then(|| (key.to_owned(), Json::String(value.to_owned())))
            })
            .collect()
    }

    /// Every key (in [`KEYS`] order) with its effective value and source. An invalid env value is an
    /// `Error::InvalidPreferences` mentioning "environment variable".
    pub fn resolve(&self) -> Result<Vec<Resolved>> {
        let (stored, env) = (self.stored()?, self.overrides());
        let mut merged = stored.clone();
        merged.extend(env.clone());
        // The stored values were checked by `stored`: an error here comes from a variable.
        let values = normalize_all(&merged, "environment variable")?;
        let defaults = defaults()?;
        let resolved = KEYS.iter().map(|&key| {
            let default = defaults.get(key).cloned().unwrap_or(Json::Null);
            let source = if env.contains_key(key) {
                ValueSource::Env
            } else if stored.contains_key(key) {
                ValueSource::File
            } else {
                ValueSource::Default
            };
            Resolved {
                key: key.to_owned(),
                value: values.get(key).cloned().unwrap_or_else(|| default.clone()),
                default,
                source,
                env: env_name(key),
                applies: if APPLIES_AT_START.contains(&key) { Applies::Restart } else { Applies::Now },
                locked: source == ValueSource::Env,
            }
        });
        Ok(resolved.collect())
    }

    pub fn current(&self) -> Result<Preferences> {
        let values: JsonMap = self.resolve()?.into_iter().map(|r| (r.key, r.value)).collect();
        Ok(serde_json::from_value(Json::Object(values))?)
    }

    /// [`Self::current`], or the defaults when the file or an env var is invalid: capture and
    /// retention keep working on safe values while the Settings page reports the error.
    pub fn effective(&self) -> Preferences {
        self.current().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "using default preferences");
            Preferences::default()
        })
    }

    /// Set values in the file; `null` removes a key (back to the default). Returns the keys whose
    /// effective value changed. Nothing is written if any change is refused: unknown key
    /// (`InvalidPreferences`), locked by env (`Locked`, naming the variable), invalid value
    /// (`InvalidPreferences`). The file is written sorted, indented, atomically, mode 0600.
    pub fn update(&self, changes: &JsonMap) -> Result<Vec<String>> {
        let mut unknown: Vec<&str> = changes.keys().map(String::as_str).filter(|key| !KEYS.contains(key)).collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(Error::InvalidPreferences(format!("unknown setting(s): {}", unknown.join(", "))));
        }
        let before = self.resolve()?;
        let mut locked: Vec<&Resolved> = before.iter().filter(|r| r.locked && changes.contains_key(&r.key)).collect();
        if !locked.is_empty() {
            locked.sort_unstable_by(|a, b| a.key.cmp(&b.key));
            let names: Vec<&str> = locked.iter().map(|r| r.env.as_str()).collect();
            return Err(Error::Locked(format!("set by an environment variable ({}); unset it to change this here", names.join(", "))));
        }
        let mut stored = self.stored()?;
        for (key, value) in changes {
            match value {
                Json::Null => stored.remove(key),
                value => stored.insert(key.clone(), value.clone()),
            };
        }
        let clean: BTreeMap<String, Json> = normalize_all(&stored, "setting")?.into_iter().collect();
        atomic_write(&self.path, &(serde_json::to_string_pretty(&clean)? + "\n"), 0o600)?;
        let after = self.resolve()?;
        Ok(before.into_iter().zip(after).filter(|(b, a)| b.value != a.value).map(|(_, a)| a.key).collect())
    }
}

/// Every default value, by key.
fn defaults() -> Result<JsonMap> {
    match serde_json::to_value(Preferences::default())? {
        Json::Object(map) => Ok(map),
        _ => Ok(JsonMap::new()),
    }
}

/// Every value in its canonical JSON form, or one `InvalidPreferences` listing each problem as
/// `<origin> <key>: <why>`.
fn normalize_all(data: &JsonMap, origin: &str) -> Result<JsonMap> {
    let mut clean = JsonMap::new();
    let mut problems = Vec::new();
    for (key, value) in data {
        match normalize(key, value) {
            Ok(value) => {
                clean.insert(key.clone(), value);
            }
            Err(why) => problems.push(format!("{origin} {key}: {why}")),
        }
    }
    if problems.is_empty() { Ok(clean) } else { Err(Error::InvalidPreferences(problems.join("; "))) }
}

/// One value in its canonical form. Lenient like the settings page's inputs and env vars need: a
/// number or boolean may be given as text, a list as comma-separated text.
fn normalize(key: &str, value: &Json) -> std::result::Result<Json, String> {
    match key {
        "default_range" => one_of(value, &["24h", "7d", "30d", "all"]),
        "week_start" => one_of(value, &["monday", "sunday"]),
        "event_retention_days" => retention_days(value),
        "hide_secrets_on_ingest" => boolean(value).map(Json::Bool),
        "disabled_sources" => string_list(value),
        _ => Err("unknown setting".into()),
    }
}

fn one_of(value: &Json, allowed: &[&str]) -> std::result::Result<Json, String> {
    match value.as_str() {
        Some(text) if allowed.contains(&text) => Ok(value.clone()),
        _ => Err(format!("must be one of {}, got {value}", allowed.join(", "))),
    }
}

fn retention_days(value: &Json) -> std::result::Result<Json, String> {
    const RANGE: std::ops::RangeInclusive<i64> = 1..=3650;
    let days = match value {
        Json::Null => return Ok(Json::Null),
        Json::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 1e15).map(whole_f64_to_i64)),
        Json::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    };
    match days {
        Some(days) if RANGE.contains(&days) => Ok(Json::from(days)),
        Some(_) => Err(format!("must be between {} and {} days, got {value}", RANGE.start(), RANGE.end())),
        None => Err(format!("must be a whole number of days or null, got {value}")),
    }
}

fn boolean(value: &Json) -> std::result::Result<bool, String> {
    let parsed = match value {
        Json::Bool(b) => Some(*b),
        Json::Number(n) => match n.as_i64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        Json::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" | "t" => Some(true),
            "false" | "0" | "no" | "n" | "off" | "f" => Some(false),
            _ => None,
        },
        _ => None,
    };
    parsed.ok_or_else(|| format!("must be true or false, got {value}"))
}

fn string_list(value: &Json) -> std::result::Result<Json, String> {
    match value {
        Json::String(text) => Ok(text.split(',').map(str::trim).filter(|item| !item.is_empty()).collect::<Vec<_>>().into()),
        Json::Array(items) if items.iter().all(Json::is_string) => Ok(value.clone()),
        _ => Err(format!("must be a list of connector ids, got {value}")),
    }
}

/// A whole `f64` already checked to lie within ±1e15, as `i64`: exact, since 1e15 < 2^53 < `i64::MAX`.
#[expect(clippy::cast_possible_truncation, reason = "the caller bounds the value to ±1e15 with no fraction")]
fn whole_f64_to_i64(f: f64) -> i64 {
    f as i64
}
