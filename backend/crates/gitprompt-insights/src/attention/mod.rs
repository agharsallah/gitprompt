//! What needs you: the sessions to pick up and up to three things worth a look. One pure source
//! for every reader (the Overview's first screen, notifications, the weekly recap, the tray menu).
//!
//! Worth a look is a list of [`Source`]s, each a small function that finds at most one [`Item`].
//! The list's order is the order of kinds on screen (nothing is ranked by a hidden score), and
//! the feed keeps the first [`WORTH_A_LOOK`]. A new kind is one function and one line in
//! [`SOURCES`]; a new reader only reads [`Attention`].

mod pick_up;
mod repeated;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use pick_up::{PICK_UP, PICK_UP_WITHIN, PickUp, Presence, RUNNING_WINDOW, SessionState, pick_up};
pub use repeated::{COMMAND_MIN_CHARS, COMMAND_MIN_SENDS, COMMAND_MIN_SESSIONS, RepeatedText, command_name};

use crate::target::Target;

/// Items Worth a look shows at most.
pub const WORTH_A_LOOK: usize = 3;

/// What kind of thing an item is; the dashboard picks its button by kind and shows a kind it does
/// not know as a plain link to its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "WorthKind")]
pub enum Kind {
    /// A prompt you send often enough to become a slash command.
    RepeatedPrompt,
}

/// What the item's one button does. GitPrompt only hands over what to copy; it never writes a file
/// or runs anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(rename = "WorthAction")]
pub enum Action {
    /// Copy a ready command file: save `text` as `<name>.md` among the tool's commands.
    CopyCommand { name: String, text: String },
}

/// One row of Worth a look: one sentence and one action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "WorthItem")]
pub struct Item {
    pub kind: Kind,
    pub text: String,
    pub action: Action,
    /// Where the row leads (also what a kind the reader does not know links to).
    pub target: Target,
}

/// What the sources read: recent sessions, whatever the range, and the range's figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// The latest sessions, any order.
    pub sessions: Vec<SessionState>,
    /// Typed texts sent more than once in the range, most sent first.
    pub repeated: Vec<RepeatedText>,
}

/// A source of Worth a look: at most one item from the inputs.
pub type Source = fn(&Inputs) -> Option<Item>;

/// Every source, in the order their kinds are shown.
pub const SOURCES: &[Source] = &[repeated::source];

/// `GET /api/attention`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Attention {
    /// Running sessions first, then the ones you left most recently ([`PICK_UP`] at most).
    pub pick_up: Vec<PickUp>,
    /// At most [`WORTH_A_LOOK`], in the order of [`SOURCES`].
    pub worth_a_look: Vec<Item>,
    /// Nothing to pick up and nothing worth a look: the calm state, said by the daemon so no reader
    /// has to interpret two empty lists.
    pub nothing_needs_you: bool,
}

/// The items of `sources`, in their order, at most [`WORTH_A_LOOK`].
pub fn worth_a_look(inputs: &Inputs, sources: &[Source]) -> Vec<Item> {
    sources.iter().filter_map(|source| source(inputs)).take(WORTH_A_LOOK).collect()
}

/// The feed at `now` from every source.
pub fn attention(inputs: &Inputs, now: Timestamp) -> Attention {
    attention_from(inputs, now, SOURCES)
}

/// [`attention`] over the given sources (tests add a source without touching [`SOURCES`]).
pub fn attention_from(inputs: &Inputs, now: Timestamp, sources: &[Source]) -> Attention {
    let pick_up = pick_up(&inputs.sessions, now);
    let worth_a_look = worth_a_look(inputs, sources);
    let nothing_needs_you = pick_up.is_empty() && worth_a_look.is_empty();
    Attention { pick_up, worth_a_look, nothing_needs_you }
}
