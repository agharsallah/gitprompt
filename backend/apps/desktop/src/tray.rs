//! The tray icon. Its menu is live: whether GitPrompt records, today's prompts and cost, the
//! sessions to pick up (a click copies the resume command) and the last prompts (a click copies
//! the prompt), then "Open GitPrompt", "Start at login", "Quit GitPrompt". The live part is
//! `gitprompt_tauri::tray_menu` (data, tested there); this file only draws it and runs the clicks.
//!
//! It is redrawn when the daemon reports a change (its own subscription, the plugin's events): a
//! capture, a leak decision, a status change. No web view and no polling; a burst of changes is
//! one or two redraws ([`Coalesce`]). The Dock badge (open possible leaks) is set with each redraw.

use std::sync::{Mutex, PoisonError};

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

use gitprompt_tauri::{CAPTURED_EVENT, Coalesce, GitPromptExt, STATUS_EVENT, TrayAction, TrayItem, TrayMenu};

use crate::{badge, window};

const TRAY: &str = "gitprompt";
const OPEN: &str = "open";
const AUTOSTART: &str = "autostart";
const QUIT: &str = "quit";
/// The ids of the live items start with this; the rest is the item's id in the [`TrayMenu`].
const LIVE: &str = "live:";

/// The menu shown now, and the redraws asked for.
#[derive(Default)]
struct Live {
    coalesce: Coalesce,
    menu: Mutex<Option<TrayMenu>>,
}

impl Live {
    fn menu(&self) -> Option<TrayMenu> {
        self.menu.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

pub(crate) fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.manage(Live::default());
    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip("GitPrompt")
        .menu(&draw(app, None)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, &event));
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    for event in [CAPTURED_EVENT, STATUS_EVENT] {
        let handle = app.clone();
        app.listen_any(event, move |_| refresh(&handle));
    }
    refresh(app);
    Ok(())
}

/// Read the menu again and draw it; while a read runs, changes wait for one more.
fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let Some(gitprompt) = app.gitprompt() else { return };
    if !app.state::<Live>().coalesce.changed() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let menu = gitprompt.tray_menu().await;
            show(&app, menu);
            if !app.state::<Live>().coalesce.finished() {
                return;
            }
        }
    });
}

fn show<R: Runtime>(app: &AppHandle<R>, menu: TrayMenu) {
    redraw(app, Some(&menu));
    badge::set(app, menu.badge);
    *app.state::<Live>().menu.lock().unwrap_or_else(PoisonError::into_inner) = Some(menu);
}

/// Put a newly drawn menu on the tray icon.
fn redraw<R: Runtime>(app: &AppHandle<R>, live: Option<&TrayMenu>) {
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    if let Err(e) = draw(app, live).and_then(|menu| tray.set_menu(Some(menu))) {
        tracing::warn!(error = %e, "could not update the tray menu");
    }
}

/// The live items (none before the first read), then the fixed ones.
fn draw<R: Runtime>(app: &AppHandle<R>, live: Option<&TrayMenu>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    if let Some(live) = live {
        for item in &live.items {
            menu.append(&*live_item(app, item)?)?;
        }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    let at_login = app.autolaunch().is_enabled().unwrap_or(false);
    menu.append(&MenuItem::with_id(app, OPEN, "Open GitPrompt", true, None::<&str>)?)?;
    menu.append(&CheckMenuItem::with_id(app, AUTOSTART, "Start at login", true, at_login, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, QUIT, "Quit GitPrompt", true, None::<&str>)?)?;
    Ok(menu)
}

fn live_item<R: Runtime>(app: &AppHandle<R>, item: &TrayItem) -> tauri::Result<Box<dyn IsMenuItem<R>>> {
    Ok(match item {
        TrayItem::Text(text) => Box::new(MenuItem::new(app, text, false, None::<&str>)?),
        TrayItem::Separator => Box::new(PredefinedMenuItem::separator(app)?),
        TrayItem::Action { id, label, .. } => Box::new(MenuItem::with_id(app, format!("{LIVE}{id}"), label, true, None::<&str>)?),
    })
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    let id = event.id().as_ref();
    if let Some(live) = id.strip_prefix(LIVE) {
        return run(app, live);
    }
    match id {
        OPEN => window::show(app),
        AUTOSTART => toggle_autostart(app),
        // Exits the event loop: the plugin stops the daemon on `RunEvent::Exit`.
        QUIT => app.exit(0),
        _ => {}
    }
}

/// A live item was clicked: copy its text or open its page.
fn run<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let action = app.state::<Live>().menu().and_then(|menu| menu.action(id).cloned());
    match action {
        Some(TrayAction::Copy(text)) => {
            if let Err(e) = app.clipboard().write_text(text) {
                tracing::warn!(error = %e, "could not copy from the tray");
            }
        }
        Some(TrayAction::Open(route)) => window::show_at(app, Some(&route)),
        None => {}
    }
}

/// Turn the login item over, then redraw: the check mark shows what the system now has.
fn toggle_autostart<R: Runtime>(app: &AppHandle<R>) {
    let launcher = app.autolaunch();
    let changed = if launcher.is_enabled().unwrap_or(false) { launcher.disable() } else { launcher.enable() };
    if let Err(e) = changed {
        tracing::warn!(error = %e, "could not change the login item");
    }
    redraw(app, app.state::<Live>().menu().as_ref());
}
