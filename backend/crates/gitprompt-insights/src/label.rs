//! The default name of a session: what you first typed in it, short enough for a list row. The
//! name you give it replaces this wherever it is shown (`gitprompt_store::store::NameKind`).

use crate::text::words;

/// Most characters of a label, the `…` of a cut one included.
pub const LABEL_CHARS: usize = 60;
const ELLIPSIS: char = '…';

/// A leading word that is a slash command (`/review`) or a path: rooted (`/src/lib.rs`, `~/notes`,
/// `./a`) or relative with a file name (`src/lib.rs`) or two folders (`a/b/c`). A word with one
/// slash and no dot (`and/or`) is a word.
fn is_command_or_path(word: &str) -> bool {
    if ["/", "~/", "./", "../"].iter().any(|prefix| word.starts_with(prefix)) {
        return true;
    }
    let file = word.rsplit('/').next().unwrap_or(word);
    word.matches('/').count() >= 2 || (word.contains('/') && file.contains('.'))
}

/// `text` on one line, cut at a word boundary to [`LABEL_CHARS`] with a `…`; `None` when it has no
/// words. A first word that alone is too long is cut inside it.
fn clip(text: &str) -> Option<String> {
    let parts: Vec<&str> = words(text).collect();
    let joined = parts.join(" ");
    if joined.chars().count() <= LABEL_CHARS {
        return (!joined.is_empty()).then_some(joined);
    }
    let room = LABEL_CHARS - 1;
    let mut cut = String::new();
    for word in parts {
        let next = if cut.is_empty() { word.chars().count() } else { cut.chars().count() + 1 + word.chars().count() };
        if next > room {
            break;
        }
        if !cut.is_empty() {
            cut.push(' ');
        }
        cut.push_str(word);
    }
    if cut.is_empty() {
        cut = joined.chars().take(room).collect();
    }
    cut.push(ELLIPSIS);
    Some(cut)
}

/// The label of one typed prompt: its words without the slash commands and paths it starts with
/// (`/review src/lib.rs fix the parser` is `fix the parser`), on one line, at most [`LABEL_CHARS`]
/// characters. `None` for a prompt with nothing else (empty, or only a command).
pub fn label(typed: &str) -> Option<String> {
    let rest: Vec<&str> = words(typed).skip_while(|word| is_command_or_path(word)).collect();
    clip(&rest.join(" "))
}

/// The default name of a session: the title its tool reported if it has one (the tool already
/// named it), else the label of the first typed prompt that has one.
pub fn session_label<'a>(title: Option<&str>, typed: impl IntoIterator<Item = &'a str>) -> Option<String> {
    title.and_then(clip).or_else(|| typed.into_iter().find_map(label))
}
