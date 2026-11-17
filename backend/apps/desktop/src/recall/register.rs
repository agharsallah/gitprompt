//! Choosing the shortcut that opens the recall window: parse it, take it, give the old one back.
//! Pure over a [`Registrar`], so the conflict path is tested without an operating system.

use gitprompt_core::shortcut;

/// What can take and release a global shortcut (the global-shortcut plugin, or a test double).
/// Both calls take the canonical text, such as `CmdOrCtrl+Alt+P`; an error is the reason.
pub(crate) trait Registrar {
    fn register(&self, shortcut: &str) -> Result<(), String>;
    fn unregister(&self, shortcut: &str) -> Result<(), String>;
}

/// Why a shortcut could not be set. `Display` is the sentence the Settings page shows.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ApplyError {
    /// Not a shortcut at all (see [`shortcut::ShortcutError`]).
    Invalid(String),
    /// A valid shortcut the system would not give: most often another app holds it.
    Taken { shortcut: String, reason: String },
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(why) => f.write_str(why),
            Self::Taken { shortcut, reason } => write!(f, "{shortcut} is not available: another app may already use it ({reason})"),
        }
    }
}

/// Make `wanted` the shortcut, given the one held now (`current`, canonical). Returns the canonical
/// text now held. A new shortcut is taken before the old one is released, so a refusal leaves the
/// old one working; asking for the one already held does nothing.
pub(crate) fn apply(registrar: &impl Registrar, current: Option<&str>, wanted: &str) -> Result<String, ApplyError> {
    let canonical = shortcut::normalize(wanted).map_err(|e| ApplyError::Invalid(e.to_string()))?;
    if current == Some(canonical.as_str()) {
        return Ok(canonical);
    }
    registrar.register(&canonical).map_err(|reason| ApplyError::Taken { shortcut: canonical.clone(), reason })?;
    if let Some(old) = current
        && let Err(e) = registrar.unregister(old)
    {
        tracing::warn!(shortcut = old, error = %e, "could not release the previous recall shortcut");
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Holds shortcuts in memory; `taken` are held by "another app".
    #[derive(Default)]
    struct Fake {
        held: RefCell<Vec<String>>,
        taken: Vec<&'static str>,
    }

    impl Registrar for Fake {
        fn register(&self, shortcut: &str) -> Result<(), String> {
            if self.taken.contains(&shortcut) {
                return Err("HotKey already registered".into());
            }
            self.held.borrow_mut().push(shortcut.to_owned());
            Ok(())
        }

        fn unregister(&self, shortcut: &str) -> Result<(), String> {
            self.held.borrow_mut().retain(|s| s != shortcut);
            Ok(())
        }
    }

    #[test]
    fn the_first_shortcut_is_registered_in_canonical_spelling() {
        let fake = Fake::default();
        assert_eq!(apply(&fake, None, "option+cmd+p").as_deref(), Ok("Alt+Super+P"));
        assert_eq!(*fake.held.borrow(), ["Alt+Super+P"]);
    }

    #[test]
    fn a_new_shortcut_replaces_the_old_one() {
        let fake = Fake::default();
        let first = apply(&fake, None, "Ctrl+Alt+P").unwrap();
        assert_eq!(apply(&fake, Some(&first), "Ctrl+Alt+K").as_deref(), Ok("Ctrl+Alt+K"));
        assert_eq!(*fake.held.borrow(), ["Ctrl+Alt+K"]);
    }

    #[test]
    fn asking_for_the_shortcut_already_held_changes_nothing() {
        let fake = Fake::default();
        assert_eq!(apply(&fake, Some("Ctrl+Alt+P"), "alt+ctrl+p").as_deref(), Ok("Ctrl+Alt+P"));
        assert!(fake.held.borrow().is_empty(), "no second registration");
    }

    #[test]
    fn a_shortcut_another_app_holds_is_reported_and_the_old_one_keeps_working() {
        let fake = Fake { taken: vec!["Ctrl+Alt+K"], ..Fake::default() };
        let first = apply(&fake, None, "Ctrl+Alt+P").unwrap();
        let refused = apply(&fake, Some(&first), "Ctrl+Alt+K").unwrap_err();
        assert_eq!(refused, ApplyError::Taken { shortcut: "Ctrl+Alt+K".into(), reason: "HotKey already registered".into() });
        assert_eq!(refused.to_string(), "Ctrl+Alt+K is not available: another app may already use it (HotKey already registered)");
        assert_eq!(*fake.held.borrow(), ["Ctrl+Alt+P"], "the previous shortcut is still held");
    }

    #[test]
    fn text_that_is_not_a_shortcut_never_reaches_the_system() {
        let fake = Fake::default();
        let refused = apply(&fake, None, "P").unwrap_err();
        assert!(matches!(refused, ApplyError::Invalid(_)), "{refused:?}");
        assert!(refused.to_string().contains("start with a modifier"));
        assert!(fake.held.borrow().is_empty());
    }
}
