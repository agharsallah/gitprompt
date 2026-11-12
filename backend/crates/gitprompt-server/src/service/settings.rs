//! The Settings and Security pages: preferences, the receiver token and its rotation.

use gitprompt_capture::rotation;
use gitprompt_core::{Error, JsonMap};

use super::data::modified;
use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::{Rotation, SecurityOut, SettingOut, SettingsOut, SettingsSaved};

impl Inner {
    fn settings_out(&self) -> ApiResult<SettingsOut> {
        let settings = self
            .preferences
            .resolve()?
            .into_iter()
            .map(|r| {
                let out =
                    SettingOut { value: r.value, default: r.default, source: r.source, env: r.env, locked: r.locked, applies: r.applies };
                (r.key, out)
            })
            .collect();
        Ok(SettingsOut { file: self.preferences.path.display().to_string(), settings })
    }
}

impl Service {
    pub async fn preferences(&self) -> ApiResult<SettingsOut> {
        self.run(Inner::settings_out).await
    }

    pub async fn update_preferences(&self, changes: JsonMap) -> ApiResult<SettingsSaved> {
        self.run(move |inner| {
            let changed = inner.preferences.update(&changes)?;
            let restart_required = changed.iter().any(|key| inner.options.applies_at_start.contains(key));
            Ok(SettingsSaved { current: inner.settings_out()?, changed, restart_required })
        })
        .await
    }

    pub async fn security(&self) -> ApiResult<SecurityOut> {
        self.run(|inner| {
            inner.settings.token()?;
            let token_file = inner.settings.token_file();
            let token_changed = modified(&std::fs::metadata(&token_file).map_err(Error::from)?);
            let set = inner.spec_set();
            let connected = set.specs.values().filter(|spec| inner.connection(spec).connected()).map(|spec| spec.name.clone()).collect();
            Ok(SecurityOut { token_file: token_file.display().to_string(), token_changed, connected })
        })
        .await
    }

    pub async fn rotate_token(&self, dry_run: bool) -> ApiResult<Rotation> {
        self.run(move |inner| {
            let set = inner.spec_set();
            Ok(rotation::rotate_via(&set.specs, &inner.settings, dry_run, inner.options.forwarder.as_deref())?.into())
        })
        .await
    }
}
