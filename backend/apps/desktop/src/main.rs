//! The GitPrompt desktop app: the [`gitprompt_tauri`] plugin runs the daemon in-process (hooks
//! reach it on `http://127.0.0.1:<port>`), the main window shows the dashboard the daemon serves,
//! the tray icon keeps capture running while the window is closed.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod perf;
mod recall;
mod tray;
mod window;

use tauri_plugin_autostart::MacosLauncher;

use gitprompt_core::preferences::PreferenceFile;
use gitprompt_core::{Env, Settings, shortcut};
use gitprompt_server::logging::{LogConfig, desktop_log_file};
use gitprompt_tauri::{PluginConfig, sidecar_next_to_exe};

/// Passed by the login item (autostart): start in the tray, without a window.
const HIDDEN_ARG: &str = "--hidden";

fn main() {
    let main_at = perf::epoch_ms();
    // The dashboard is the copy compiled into the app (read from `web/build` in debug builds);
    // `GITPROMPT_DASHBOARD` serves another folder.
    let config = PluginConfig { forwarder: sidecar_next_to_exe(), ..PluginConfig::default() };
    let settings = config.settings().ok();
    init_logging(settings.as_ref());
    let perf = perf::from_settings(settings.as_ref(), main_at).unwrap_or_else(|e| {
        tracing::error!(error = %e, "GitPrompt cannot run its performance checks");
        std::process::exit(2);
    });
    let hidden = std::env::args().any(|arg| arg == HIDDEN_ARG) && perf.is_none();
    let performance_run = perf.is_some();
    let recall_shortcut = settings
        .as_ref()
        .map_or_else(|| shortcut::DEFAULT.to_owned(), |s| PreferenceFile::new(&s.home, s.env.clone()).effective().recall_shortcut);
    let mut builder = tauri::Builder::default();
    if let Some(perf) = perf {
        builder = builder.manage(perf);
    } else {
        // First, so a second launch only brings the running app forward. Not in a performance run,
        // which must start even while an installed GitPrompt runs.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::show(app)));
    }
    let app = builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(recall::plugin())
        .plugin(gitprompt_tauri::init(config))
        // The dashboard's native calls beyond the plugin's: `platform.hideWindow()` and the recall
        // shortcut (permissions: `capabilities/`, names in `build.rs`).
        .invoke_handler(tauri::generate_handler![window::hide_window, recall::recall_register, recall::recall_status])
        .setup(move |app| {
            perf::mark(app.handle(), "setup");
            tray::build(app.handle())?;
            recall::setup(app.handle(), &recall_shortcut, !performance_run);
            if !hidden {
                window::show(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            window::on_event(window, event);
            recall::on_event(window, event);
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|app, event| {
            // macOS: clicking the Dock icon while the window is hidden (`Reopen` is macOS-only).
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                window::show(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
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
fn init_logging(settings: Option<&Settings>) {
    let log = match settings {
        Some(settings) => LogConfig::from_env(&settings.env).with_file(desktop_log_file(&settings.home)),
        None => LogConfig::from_env(&Env::process()),
    };
    if let Err(e) = gitprompt_server::logging::init(&log) {
        // Already logged on stderr by `init`; keep the reason for whoever reads the terminal.
        eprintln!("GitPrompt: logging to stderr only: {e}");
    }
    let profile = settings.map(|s| s.profile).unwrap_or_default();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), %profile, file = ?log.file, "GitPrompt desktop starting");
}
