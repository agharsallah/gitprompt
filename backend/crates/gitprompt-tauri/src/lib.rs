//! The GitPrompt Tauri v2 plugin: the reusable integration point between a Tauri shell and the
//! GitPrompt backend.
//!
//! While the app is open the plugin runs the HTTP daemon ([`gitprompt_server::Daemon`]) in-process
//! on Tauri's async runtime, because the AI tools deliver hooks by POSTing to
//! `http://127.0.0.1:<port>/ingest/<source>` (or by running the bundled `gitprompt hook <source>`
//! forwarder, which spools while the app is closed; the daemon drains that spool at start).
//!
//! * [`init`] builds the plugin from a [`PluginConfig`].
//! * [`GitPrompt`] (managed state, also via [`GitPromptExt`]) tells the shell the daemon's
//!   [`DaemonStatus`] and URL; [`GitPrompt::ready`] waits for the start to settle.
//! * Live-update notices are emitted as [`CAPTURED_EVENT`] (`{prompts, events}`), status changes as
//!   [`STATUS_EVENT`].
//! * Native commands (`plugin:gitprompt|<name>`, see [`COMMANDS`]) are thin over
//!   [`gitprompt_server::Service`]; errors serialize as `{status, detail}` ([`CommandError`]).
//! * On `RunEvent::Exit` the daemon shuts down gracefully.
//!
//! When another process (a CLI `gitprompt serve`) already holds the data directory's lock, the
//! plugin does not fail: the status is [`DaemonStatus::External`] and the shell points its window
//! at that daemon's URL instead.

mod command_names;
mod commands;
mod config;
mod error;
mod lifecycle;
mod native;
mod state;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, RunEvent, Runtime};

pub use command_names::COMMANDS;
pub use commands::{ExportSaved, StatusOut};
pub use config::{PluginConfig, sidecar_next_to_exe};
pub use error::CommandError;
pub use lifecycle::{CAPTURED_EVENT, STATUS_EVENT};
pub use native::{Opener, SystemOpener};
pub use state::{DaemonStatus, GitPrompt};

/// The plugin's name: commands are invoked as `plugin:gitprompt|<command>`, permissions are
/// `gitprompt:default`, `gitprompt:allow-<command>`.
pub const PLUGIN_NAME: &str = "gitprompt";

/// The plugin: settings from the process environment overridden by `config`, the daemon started
/// in the background at setup (see [`GitPrompt::ready`]), stopped at exit.
///
/// Setup fails only when the settings can't be built (no `HOME`, a `GITPROMPT_PORT` that is not a
/// port number). A daemon that can't start leaves the app running with [`DaemonStatus::Failed`].
pub fn init<R: Runtime>(config: PluginConfig) -> TauriPlugin<R> {
    Builder::new(PLUGIN_NAME)
        .invoke_handler(commands::handler())
        .setup(move |app, _api| {
            let settings = config.settings()?;
            let options = config.app_options();
            let state = GitPrompt::new(settings, config.opener());
            app.manage(state.clone());
            tauri::async_runtime::spawn(lifecycle::start(app.clone(), state, options, config.daemon_options()));
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Exit = event {
                lifecycle::exit(app);
            }
        })
        .build()
}

/// Access to the plugin's state from anything that manages Tauri state (`App`, `AppHandle`,
/// windows). `None` before the plugin's setup ran.
pub trait GitPromptExt<R: Runtime> {
    fn gitprompt(&self) -> Option<GitPrompt>;
}

impl<R: Runtime, M: Manager<R>> GitPromptExt<R> for M {
    fn gitprompt(&self) -> Option<GitPrompt> {
        self.try_state::<GitPrompt>().map(|state| state.inner().clone())
    }
}
