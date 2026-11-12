//! The Settings page: every preference with its value, default and where it comes from.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::Json;
use gitprompt_core::preferences::{Applies, ValueSource};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SettingOut {
    pub value: Json,
    pub default: Json,
    pub source: ValueSource,
    pub env: String,
    pub locked: bool,
    pub applies: Applies,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "SettingsState")]
pub struct SettingsOut {
    /// `<home>/preferences.json`.
    pub file: String,
    /// Every preference key, in `KEYS` order.
    pub settings: IndexMap<String, SettingOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SettingsSaved {
    #[serde(flatten)]
    pub current: SettingsOut,
    pub changed: Vec<String>,
    pub restart_required: bool,
}
