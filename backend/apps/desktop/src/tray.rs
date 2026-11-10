//! The tray icon: "Open GitPrompt", "Start at login", "Quit GitPrompt".

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::window;

const OPEN: &str = "open";
const AUTOSTART: &str = "autostart";
const QUIT: &str = "quit";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, OPEN, "Open GitPrompt", true, None::<&str>)?;
    let at_login = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(app, AUTOSTART, "Start at login", true, at_login, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit GitPrompt", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &autostart, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut tray = TrayIconBuilder::with_id("gitprompt")
        .tooltip("GitPrompt")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| on_menu(app, &event, &autostart));
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent, autostart: &CheckMenuItem<R>) {
    match event.id().as_ref() {
        OPEN => window::show(app),
        AUTOSTART => toggle_autostart(app, autostart),
        // Exits the event loop: the plugin stops the daemon on `RunEvent::Exit`.
        QUIT => app.exit(0),
        _ => {}
    }
}

/// The item toggles itself when clicked; follow it, and put it back when the login item can't change.
fn toggle_autostart<R: Runtime>(app: &AppHandle<R>, item: &CheckMenuItem<R>) {
    let launcher = app.autolaunch();
    let wanted = item.is_checked().unwrap_or(false);
    let changed = if wanted { launcher.enable() } else { launcher.disable() };
    if let Err(e) = changed {
        tracing::warn!(error = %e, "could not change the login item");
        let _ = item.set_checked(launcher.is_enabled().unwrap_or(false));
    }
}
