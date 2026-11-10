//! The GitPrompt desktop app: the [`gitprompt_tauri`] plugin runs the daemon in-process (hooks
//! reach it on `http://127.0.0.1:<port>`), the main window shows the dashboard the daemon serves,
//! the tray icon keeps capture running while the window is closed.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod window;

use tauri::RunEvent;
use tauri_plugin_autostart::MacosLauncher;

use gitprompt_core::Env;
use gitprompt_server::logging::{LogConfig, desktop_log_file};
use gitprompt_tauri::{PluginConfig, sidecar_next_to_exe};

/// Passed by the login item (autostart): start in the tray, without a window.
const HIDDEN_ARG: &str = "--hidden";

fn main() {
    // The dashboard is the copy compiled into the app (read from `web/build` in debug builds);
    // `GITPROMPT_DASHBOARD` serves another folder.
    let config = PluginConfig { forwarder: sidecar_next_to_exe(), ..PluginConfig::default() };
    init_logging(&config);
    let hidden = std::env::args().any(|arg| arg == HIDDEN_ARG);
    let app = tauri::Builder::default()
        // First, so a second launch only brings the running app forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::show(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(gitprompt_tauri::init(config))
        .setup(move |app| {
            tray::build(app.handle())?;
            if !hidden {
                window::show(app.handle());
            }
            Ok(())
        })
        .on_window_event(window::on_event)
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|app, event| {
            // macOS: clicking the Dock icon while the window is hidden.
            if let RunEvent::Reopen { .. } = event {
                window::show(app);
            }
        }),
        Err(e) => {
            tracing::error!(error = %e, "GitPrompt could not start");
            std::process::exit(1);
        }
    }
}

/// Logs on stderr and in `<data dir>/logs/desktop.log` (`GITPROMPT_LOG_LEVEL`,
/// `GITPROMPT_LOG_FORMAT`, as the CLI reads them). Without usable settings (no `HOME`), stderr only;
/// the plugin reports that settings error at setup.
fn init_logging(config: &PluginConfig) {
    let log = match config.settings() {
        Ok(settings) => LogConfig::from_env(&settings.env).with_file(desktop_log_file(&settings.home)),
        Err(_) => LogConfig::from_env(&Env::process()),
    };
    if let Err(e) = gitprompt_server::logging::init(&log) {
        // Already logged on stderr by `init`; keep the reason for whoever reads the terminal.
        eprintln!("GitPrompt: logging to stderr only: {e}");
    }
    tracing::info!(version = env!("CARGO_PKG_VERSION"), file = ?log.file, "GitPrompt desktop starting");
}
