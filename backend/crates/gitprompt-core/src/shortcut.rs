//! The recall shortcut as text: `CmdOrCtrl+Alt+P`. Pure parsing and one canonical spelling, so the
//! preference file, the Settings page and the desktop app all agree on what a valid shortcut is.
//! The desktop app turns the canonical text into a key press for the operating system.
//!
//! A shortcut is one or more modifiers and exactly one key, in any order and case. A key alone is
//! refused: a global shortcut without a modifier would take that key from every other app.

use std::collections::BTreeMap;

/// The shortcut the recall window opens with: ⌥⌘P on macOS, Ctrl Alt P elsewhere.
pub const DEFAULT: &str = "CmdOrCtrl+Alt+P";

/// Modifiers in the order the canonical text writes them, each with the spellings it is read from.
const MODIFIERS: &[(&str, &[&str])] = &[
    ("CmdOrCtrl", &["CMDORCTRL", "CMDORCONTROL", "COMMANDORCTRL", "COMMANDORCONTROL"]),
    ("Ctrl", &["CTRL", "CONTROL"]),
    ("Alt", &["ALT", "OPTION"]),
    ("Shift", &["SHIFT"]),
    ("Super", &["SUPER", "CMD", "COMMAND", "META", "WIN"]),
];

/// Why a text is not a shortcut. `Display` is the sentence the Settings page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutError {
    Empty,
    /// A `+` with nothing on one side of it.
    EmptyPart,
    /// No key at the end: only modifiers (`Ctrl+Alt`, or `P+Alt`).
    NoKey,
    /// Two keys, such as `Ctrl+A+B` (this holds the first: `A`).
    TwoKeys(String),
    /// A key alone: no modifier.
    NoModifier,
    /// A word that is neither a modifier nor a key this shortcut can use.
    UnknownKey(String),
}

impl std::fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("the shortcut is empty"),
            Self::EmptyPart => f.write_str("a key is missing between two plus signs"),
            Self::NoKey => f.write_str("add a key after the modifiers, such as P"),
            Self::TwoKeys(word) => write!(f, "only the last part can be a key, the others are modifiers ({word} is a key)"),
            Self::NoModifier => f.write_str("start with a modifier (Cmd, Ctrl, Alt or Shift) and end with the key"),
            Self::UnknownKey(word) => write!(f, "{word} is not a modifier or a key a shortcut can use (letters, digits, F1 to F24, Space)"),
        }
    }
}

impl std::error::Error for ShortcutError {}

/// A parsed shortcut: which modifiers, and the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// Canonical names, in canonical order, without repeats.
    modifiers: Vec<&'static str>,
    /// Canonical key: `A`, `7`, `F5`, `Space`.
    key: String,
}

impl Shortcut {
    /// Read a shortcut from text; spaces around each part are ignored.
    pub fn parse(text: &str) -> Result<Shortcut, ShortcutError> {
        if text.trim().is_empty() {
            return Err(ShortcutError::Empty);
        }
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err(ShortcutError::EmptyPart);
        }
        // The key is the last part; everything before it is a modifier.
        let Some((last, leading)) = parts.split_last() else { return Err(ShortcutError::Empty) };
        if modifier(last).is_some() {
            return Err(ShortcutError::NoKey);
        }
        let key = key_name(last).ok_or_else(|| ShortcutError::UnknownKey((*last).to_owned()))?;
        // By position in `MODIFIERS`: canonical order, and a repeated modifier counts once.
        let mut present: BTreeMap<usize, &'static str> = BTreeMap::new();
        for part in leading {
            let (index, name) = modifier(part).ok_or_else(|| other_key_error(part))?;
            present.insert(index, name);
        }
        if present.is_empty() {
            return Err(ShortcutError::NoModifier);
        }
        Ok(Shortcut { modifiers: present.into_values().collect(), key })
    }

    /// The canonical text, such as `CmdOrCtrl+Alt+P`: what is stored and what the OS layer parses.
    pub fn canonical(&self) -> String {
        let mut text = self.modifiers.join("+");
        text.push('+');
        text.push_str(&self.key);
        text
    }
}

/// The canonical text of a shortcut, or why it is not one.
pub fn normalize(text: &str) -> Result<String, ShortcutError> {
    Shortcut::parse(text).map(|shortcut| shortcut.canonical())
}

/// A leading part that is not a modifier: a second key, or nothing a shortcut can use.
fn other_key_error(word: &str) -> ShortcutError {
    if key_name(word).is_some() { ShortcutError::TwoKeys(word.to_owned()) } else { ShortcutError::UnknownKey(word.to_owned()) }
}

/// The modifier a word spells: its position in [`MODIFIERS`] and its canonical name.
fn modifier(word: &str) -> Option<(usize, &'static str)> {
    let upper = word.to_ascii_uppercase();
    MODIFIERS.iter().enumerate().find(|(_, (_, spellings))| spellings.contains(&upper.as_str())).map(|(index, (name, _))| (index, *name))
}

/// The canonical key for a word: a letter, a digit, `F1`..`F24` or `Space`.
fn key_name(word: &str) -> Option<String> {
    let upper = word.to_ascii_uppercase();
    let mut chars = upper.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(upper),
        (Some('F'), Some(_)) => function_key(&upper[1..]).then_some(upper),
        _ if upper == "SPACE" => Some("Space".to_owned()),
        _ => None,
    }
}

fn function_key(digits: &str) -> bool {
    !digits.starts_with('0') && digits.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n))
}
