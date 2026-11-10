//! [`PluginConfig`]: what the shell decides, mapped onto the backend's [`Settings`],
//! [`AppOptions`] and [`DaemonOptions`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitprompt_core::{Env, Settings};
use gitprompt_server::{AppOptions, DaemonOptions, Dashboard};

use crate::native::{Opener, SystemOpener};

/// How the shell wants GitPrompt to run. Explicit fields win over the environment
/// (`GITPROMPT_HOME`, `GITPROMPT_PORT`, ...), which wins over the defaults.
#[derive(Clone)]
pub struct PluginConfig {
    /// Data directory. Default: `GITPROMPT_HOME`, else `~/.gitprompt`.
    pub data_dir: Option<PathBuf>,
    /// Port of the receiver and dashboard. Default: `GITPROMPT_PORT`, else 7878. `0` binds any free
    /// port (tests; connected hooks would not find it).
    pub port: Option<u16>,
    /// The `gitprompt` executable hooks run (the bundled sidecar, see [`sidecar_next_to_exe`]).
    /// `None`: hooks are the specs' curl commands.
    pub forwarder: Option<PathBuf>,
    /// Serve the built dashboard at `/` (the window loads it from the daemon). Default `true`.
    pub serve_dashboard: bool,
    /// A built dashboard folder to serve instead of the copy compiled into the binary. Default:
    /// `GITPROMPT_DASHBOARD`, else the embedded copy. Ignored (embedded copy) when it is not a
    /// directory.
    pub dashboard_dir: Option<PathBuf>,
    /// The environment settings and specs read. Default: the process environment, read at setup.
    pub env: Option<Env>,
    /// Base options (clock, git, tool versions...). Default: [`AppOptions::default`].
    pub options: Option<AppOptions>,
    /// Opens folders and reveals files for the native commands. Default: [`SystemOpener`].
    pub opener: Option<Arc<dyn Opener>>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        PluginConfig {
            data_dir: None,
            port: None,
            forwarder: None,
            serve_dashboard: true,
            dashboard_dir: None,
            env: None,
            options: None,
            opener: None,
        }
    }
}

impl fmt::Debug for PluginConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginConfig")
            .field("data_dir", &self.data_dir)
            .field("port", &self.port)
            .field("forwarder", &self.forwarder)
            .field("serve_dashboard", &self.serve_dashboard)
            .field("dashboard_dir", &self.dashboard_dir)
            .field("env", &self.env.is_some())
            .finish_non_exhaustive()
    }
}

impl PluginConfig {
    /// Settings from [`Self::env`] (or the process environment), then the explicit overrides.
    pub fn settings(&self) -> gitprompt_core::Result<Settings> {
        let env = self.env.clone().unwrap_or_else(Env::process);
        let mut settings = Settings::from_env(env)?;
        if let Some(dir) = &self.data_dir {
            settings.home.clone_from(dir);
        }
        if let Some(port) = self.port {
            settings.port = port;
        }
        Ok(settings)
    }

    /// The server options: [`Self::options`] with the forwarder and the dashboard of this config:
    /// [`Self::dashboard`] when `serve_dashboard`, else none (API only).
    pub fn app_options(&self) -> AppOptions {
        let mut options = self.options.clone().unwrap_or_default();
        if let Some(forwarder) = &self.forwarder {
            options.forwarder = Some(forwarder.clone());
        }
        options.dashboard = self.serve_dashboard.then(|| self.dashboard());
        options
    }

    /// [`Self::dashboard_dir`], else `GITPROMPT_DASHBOARD` of [`Self::env`] (or the process
    /// environment), when it is a directory; else [`Dashboard::Embedded`].
    pub fn dashboard(&self) -> Dashboard {
        let dir = match &self.dashboard_dir {
            Some(dir) => Some(dir.clone()),
            None => match Dashboard::from_env(&self.env.clone().unwrap_or_else(Env::process)) {
                Dashboard::Dir(dir) => Some(dir),
                Dashboard::Embedded => None,
            },
        };
        match dir {
            Some(dir) if dir.is_dir() => Dashboard::Dir(dir),
            Some(dir) => {
                tracing::warn!(dir = %dir.display(), "the dashboard folder does not exist; serving the embedded dashboard");
                Dashboard::Embedded
            }
            None => Dashboard::Embedded,
        }
    }

    /// Listen on `127.0.0.1:<settings.port>`, default retention schedule.
    pub fn daemon_options(&self) -> DaemonOptions {
        DaemonOptions::default()
    }

    pub(crate) fn opener(&self) -> Arc<dyn Opener> {
        self.opener.clone().unwrap_or_else(|| Arc::new(SystemOpener))
    }
}

/// The `gitprompt` CLI next to the running executable, where Tauri puts an `externalBin` sidecar
/// (and where cargo puts it in a dev build of the workspace). `None` when there is none.
pub fn sidecar_next_to_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    sidecar_in(exe.parent()?)
}

/// `<dir>/gitprompt[.exe]` when it is a file.
pub(crate) fn sidecar_in(dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join(format!("gitprompt{}", std::env::consts::EXE_SUFFIX));
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn env(pairs: &[(&str, &str)]) -> Env {
        Env::from_pairs(pairs.iter().copied())
    }

    fn config(pairs: &[(&str, &str)]) -> PluginConfig {
        PluginConfig { env: Some(env(pairs)), ..PluginConfig::default() }
    }

    #[test]
    fn settings_default_to_the_home_folder_and_port_7878() {
        let settings = config(&[("HOME", "/u")]).settings().unwrap();
        assert_eq!(settings.home, PathBuf::from("/u/.gitprompt"));
        assert_eq!(settings.port, 7878);
    }

    #[test]
    fn settings_read_the_environment() {
        let settings = config(&[("HOME", "/u"), ("GITPROMPT_HOME", "~/data"), ("GITPROMPT_PORT", "9000")]).settings().unwrap();
        assert_eq!(settings.home, PathBuf::from("/u/data"));
        assert_eq!(settings.port, 9000);
    }

    #[test]
    fn explicit_fields_win_over_the_environment() {
        let config = PluginConfig {
            data_dir: Some("/elsewhere".into()),
            port: Some(9100),
            ..config(&[("HOME", "/u"), ("GITPROMPT_HOME", "/env"), ("GITPROMPT_PORT", "9000")])
        };
        let settings = config.settings().unwrap();
        assert_eq!(settings.home, PathBuf::from("/elsewhere"));
        assert_eq!(settings.port, 9100);
    }

    #[test]
    fn an_explicit_port_does_not_rescue_a_bad_environment_port() {
        // The environment is read first, as the CLI reads it: a broken variable is reported.
        let config = PluginConfig { port: Some(9100), ..config(&[("HOME", "/u"), ("GITPROMPT_PORT", "nope")]) };
        assert!(config.settings().unwrap_err().to_string().contains("GITPROMPT_PORT"));
    }

    #[test]
    fn settings_need_a_home() {
        assert!(config(&[]).settings().is_err());
    }

    #[test]
    fn the_forwarder_is_passed_to_the_server() {
        let with = PluginConfig { forwarder: Some("/app/gitprompt".into()), ..config(&[]) };
        assert_eq!(with.app_options().forwarder, Some(PathBuf::from("/app/gitprompt")));
        assert_eq!(config(&[]).app_options().forwarder, None);
    }

    #[test]
    fn the_embedded_dashboard_is_the_default() {
        assert_eq!(config(&[]).app_options().dashboard, Some(Dashboard::Embedded));
    }

    #[test]
    fn the_dashboard_prefers_the_explicit_dir_then_the_environment() {
        let explicit = tempfile::tempdir().unwrap();
        let from_env = tempfile::tempdir().unwrap();
        let env_dir = from_env.path().display().to_string();
        let config = PluginConfig { dashboard_dir: Some(explicit.path().into()), ..config(&[("GITPROMPT_DASHBOARD", &env_dir)]) };
        assert_eq!(config.app_options().dashboard, Some(Dashboard::Dir(explicit.path().into())));
        let config = PluginConfig { dashboard_dir: None, ..config };
        assert_eq!(config.app_options().dashboard, Some(Dashboard::Dir(from_env.path().into())));
    }

    #[test]
    fn a_missing_dashboard_dir_falls_back_to_the_embedded_dashboard() {
        let explicit = PluginConfig { dashboard_dir: Some("/no/such/dashboard".into()), ..config(&[]) };
        assert_eq!(explicit.app_options().dashboard, Some(Dashboard::Embedded));
        let from_env = config(&[("GITPROMPT_DASHBOARD", "/no/such/dashboard")]);
        assert_eq!(from_env.app_options().dashboard, Some(Dashboard::Embedded));
    }

    #[test]
    fn serve_dashboard_false_never_serves_one() {
        let dir = tempfile::tempdir().unwrap();
        let config = PluginConfig { serve_dashboard: false, dashboard_dir: Some(dir.path().into()), ..config(&[]) };
        assert_eq!(config.app_options().dashboard, None);
    }

    #[test]
    fn base_options_are_kept() {
        let base = AppOptions { bus_backlog: 7, ..AppOptions::default() };
        let config = PluginConfig { options: Some(base), ..config(&[]) };
        assert_eq!(config.app_options().bus_backlog, 7);
    }

    #[test]
    fn the_sidecar_is_found_next_to_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(sidecar_in(dir.path()), None);
        let file = dir.path().join(format!("gitprompt{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&file, b"").unwrap();
        assert_eq!(sidecar_in(dir.path()), Some(file));
    }
}
