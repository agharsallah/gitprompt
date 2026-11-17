//! The recall shortcut: from any app, a global shortcut opens a small window (`/?recall=1`: the
//! palette over prompts only, where ↵ copies) that goes away on blur and after a copy.
//!
//! Nothing runs while it is unused: the operating system holds the shortcut and wakes the app
//! when it is pressed. The shortcut comes from the `recall_shortcut` preference, is registered at
//! start and again when Settings changes it (`recall_register`); a shortcut another app holds is a
//! message in Settings, never a crash.

mod panel;
mod place;
mod register;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime, State, Window, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use self::register::{Registrar, apply};

/// What the app knows about the shortcut: the one it holds, why the wanted one is not held, and
/// whether the window has been focused since it was last shown.
#[derive(Debug, Default)]
pub(crate) struct Recall {
    held: Mutex<Option<String>>,
    problem: Mutex<Option<String>>,
    focused: AtomicBool,
}

impl Recall {
    /// The window is about to be shown: it has not been focused yet.
    fn reset(&self) {
        self.focused.store(false, Ordering::Relaxed);
    }

    /// The window's focus changed. True when it lost the focus it had: time to hide. A blur that
    /// arrives before the window ever got focus (while it is being shown) is not one.
    fn focus_changed(&self, focused: bool) -> bool {
        if focused {
            self.focused.store(true, Ordering::Relaxed);
            return false;
        }
        self.focused.swap(false, Ordering::Relaxed)
    }

    fn set(&self, held: Option<String>, problem: Option<String>) {
        if let (Some(held), Ok(mut now)) = (held, self.held.lock()) {
            *now = Some(held);
        }
        if let Ok(mut now) = self.problem.lock() {
            *now = problem;
        }
    }

    fn held(&self) -> Option<String> {
        self.held.lock().ok().and_then(|held| held.clone())
    }
}

/// The global-shortcut plugin, wired to open the window. It registers nothing until [`setup`].
pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            // Pressed only: the release of the same key must not close what the press opened.
            if event.state() == ShortcutState::Pressed {
                panel::toggle(app);
            }
        })
        .build()
}

/// Remember the shortcut state and take the wanted shortcut (`register_now` false in a performance
/// run, which must not take the shortcut from an installed GitPrompt).
pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>, wanted: &str, register_now: bool) {
    let recall = Recall::default();
    if register_now {
        match apply(&Plugin(app), None, wanted) {
            Ok(held) => recall.set(Some(held), None),
            Err(e) => {
                tracing::warn!(error = %e, "the recall shortcut is not registered");
                recall.set(None, Some(e.to_string()));
            }
        }
    }
    app.manage(recall);
}

/// Hide on blur, and keep the window (closing it would build it again next time).
pub(crate) fn on_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != panel::LABEL {
        return;
    }
    match event {
        WindowEvent::Focused(focused) => {
            let lost = window.try_state::<Recall>().is_some_and(|recall| recall.focus_changed(*focused));
            if lost && let Err(e) = window.hide() {
                tracing::warn!(error = %e, "could not hide the recall window");
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        _ => {}
    }
}

/// Settings changed the shortcut: take the new one (the old keeps working when this fails). The
/// answer is the sentence Settings shows next to the field.
#[tauri::command]
pub(crate) async fn recall_register(app: AppHandle, state: State<'_, Recall>, shortcut: String) -> Result<(), String> {
    match apply(&Plugin(&app), state.held().as_deref(), &shortcut) {
        Ok(held) => {
            state.set(Some(held), None);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Why the wanted shortcut is not held (it was refused at start), if it is not.
#[tauri::command]
pub(crate) async fn recall_status(state: State<'_, Recall>) -> Result<Option<String>, String> {
    Ok(state.problem.lock().ok().and_then(|problem| problem.clone()))
}

/// The global-shortcut plugin as a [`Registrar`].
struct Plugin<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> Registrar for Plugin<'_, R> {
    fn register(&self, shortcut: &str) -> Result<(), String> {
        self.0.global_shortcut().register(shortcut).map_err(|e| e.to_string())
    }

    fn unregister(&self, shortcut: &str) -> Result<(), String> {
        self.0.global_shortcut().unregister(shortcut).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blur_before_the_window_was_ever_focused_does_not_hide_it() {
        let recall = Recall::default();
        assert!(!recall.focus_changed(false));
    }

    #[test]
    fn losing_focus_hides_once() {
        let recall = Recall::default();
        assert!(!recall.focus_changed(true));
        assert!(recall.focus_changed(false));
        assert!(!recall.focus_changed(false), "a second blur has nothing left to hide");
    }

    #[test]
    fn showing_the_window_again_waits_for_its_focus_before_a_blur_counts() {
        let recall = Recall::default();
        recall.focus_changed(true);
        recall.reset();
        assert!(!recall.focus_changed(false));
    }

    #[test]
    fn a_refused_shortcut_keeps_the_held_one_and_says_why() {
        let recall = Recall::default();
        recall.set(Some("Ctrl+Alt+P".into()), None);
        recall.set(None, Some("taken".into()));
        assert_eq!(recall.held().as_deref(), Some("Ctrl+Alt+P"));
        assert_eq!(recall.problem.lock().unwrap().as_deref(), Some("taken"));
    }
}
