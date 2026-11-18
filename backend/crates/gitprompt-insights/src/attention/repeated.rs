//! A source of Worth a look: a prompt you type often enough, in enough sessions, to deserve a slash
//! command. Its action hands over a ready command file (a suggested name and the text) to copy.

use super::{Action, Inputs, Item, Kind};
use crate::target::Target;
use crate::text::{is_space, words};

/// Sent at least this often in the range...
pub const COMMAND_MIN_SENDS: i64 = 5;
/// ...in at least this many sessions (a word repeated inside one session is a conversation)...
pub const COMMAND_MIN_SESSIONS: i64 = 2;
/// ...and at least this long ("yes" and "continue" are answers, not commands).
pub const COMMAND_MIN_CHARS: usize = 12;

/// Characters of the quoted text in the sentence.
const QUOTE_CHARS: usize = 48;
/// Words of the text in the suggested command name, and its length at most.
const NAME_WORDS: usize = 4;
const NAME_CHARS: usize = 32;
/// Words the Prompts page searches for (search matches every word anywhere in the text).
const SEARCH_WORDS: usize = 8;

/// A typed text sent more than once in the range.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepeatedText {
    /// The whole typed text of its latest send.
    pub text: String,
    pub prompts: i64,
    pub sessions: i64,
}

impl RepeatedText {
    fn worth_a_command(&self) -> bool {
        let text = self.text.trim();
        self.prompts >= COMMAND_MIN_SENDS
            && self.sessions >= COMMAND_MIN_SESSIONS
            && text.chars().count() >= COMMAND_MIN_CHARS
            && !text.starts_with('/')
    }
}

/// The most sent text that is worth a command.
pub(super) fn source(inputs: &Inputs) -> Option<Item> {
    let found = inputs.repeated.iter().find(|r| r.worth_a_command())?;
    let text = found.text.trim();
    Some(Item {
        kind: Kind::RepeatedPrompt,
        text: format!("“{}” sent {}×", quote(text), found.prompts),
        action: Action::CopyCommand { name: command_name(text), text: text.to_owned() },
        target: Target::Prompts { q: words(text).take(SEARCH_WORDS).collect::<Vec<_>>().join(" ") },
    })
}

/// The text on one line, cut on a word with an ellipsis when long.
fn quote(text: &str) -> String {
    let line: String = text.split(is_space).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    if line.chars().count() <= QUOTE_CHARS {
        return line;
    }
    let cut: String = line.chars().take(QUOTE_CHARS).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{cut}…")
}

/// A command name from the first words: lowercase letters and digits joined by `-`, at most
/// [`NAME_CHARS`]; `repeat` when the text has no such word.
pub fn command_name(text: &str) -> String {
    let clean = |word: &str| word.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect::<String>();
    let mut name = String::new();
    for word in words(text).map(clean).filter(|w| !w.is_empty()).take(NAME_WORDS) {
        if name.len() + word.len() + 1 > NAME_CHARS {
            break;
        }
        if !name.is_empty() {
            name.push('-');
        }
        name.push_str(&word);
    }
    if name.is_empty() { "repeat".to_owned() } else { name }
}
