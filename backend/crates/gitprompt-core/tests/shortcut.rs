//! The recall shortcut's text: what is accepted, how it is spelled, and why the rest is refused.

use gitprompt_core::shortcut::{DEFAULT, Shortcut, ShortcutError, normalize};

#[test]
fn the_default_is_already_canonical() {
    assert_eq!(normalize(DEFAULT).unwrap(), DEFAULT);
}

#[test]
fn spellings_and_order_settle_into_one_canonical_text() {
    for (typed, canonical) in [("cmdorctrl+alt+p", "CmdOrCtrl+Alt+P"), ("Option + Command + P", "Alt+Super+P")] {
        assert_eq!(normalize(typed).unwrap(), canonical, "{typed}");
    }
    assert_eq!(normalize("Shift+Control+Alt+Meta+f12").unwrap(), "Ctrl+Alt+Shift+Super+F12");
    assert_eq!(normalize("ctrl+alt+space").unwrap(), "Ctrl+Alt+Space");
    assert_eq!(normalize("Alt+Alt+7").unwrap(), "Alt+7", "a repeated modifier counts once");
}

#[test]
fn each_refusal_names_its_reason() {
    let refused = |text: &str| Shortcut::parse(text).unwrap_err();
    assert_eq!(refused(""), ShortcutError::Empty);
    assert_eq!(refused("   "), ShortcutError::Empty);
    assert_eq!(refused("Alt++P"), ShortcutError::EmptyPart);
    assert_eq!(refused("Alt+"), ShortcutError::EmptyPart);
    assert_eq!(refused("Ctrl+Alt"), ShortcutError::NoKey);
    assert_eq!(refused("P"), ShortcutError::NoModifier, "a bare key would be taken from every app");
    assert_eq!(refused("P+Alt"), ShortcutError::NoKey, "the key comes last");
    assert_eq!(refused("Ctrl+A+B"), ShortcutError::TwoKeys("A".into()));
    assert_eq!(refused("Ctrl+Enter+P"), ShortcutError::UnknownKey("Enter".into()));
    assert_eq!(refused("Ctrl+Enter"), ShortcutError::UnknownKey("Enter".into()));
    assert_eq!(refused("Ctrl+F25"), ShortcutError::UnknownKey("F25".into()));
    assert_eq!(refused("Ctrl+F0"), ShortcutError::UnknownKey("F0".into()));
}

#[test]
fn a_refusal_reads_as_a_sentence_for_the_settings_page() {
    let why = Shortcut::parse("Ctrl+Alt").unwrap_err().to_string();
    assert_eq!(why, "add a key after the modifiers, such as P");
}
