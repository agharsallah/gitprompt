//! Who produced a prompt, and which part of it the user actually typed. Derived on read from the
//! source spec's `origin` rules and `injected` tags (ADR-0010), so a changed rule reclassifies old
//! prompts too, with no migration. Pure functions: no I/O, no knowledge of any tool (that lives in
//! the YAML specs).

use serde::{Deserialize, Serialize};

use gitprompt_core::spec::{Origin, OriginRule, Spec, When};
use gitprompt_store::{EventRow, PromptRow};

/// The origin when no rule matches.
pub const DEFAULT: Origin = Origin::Typed;

/// What each derived value means (listed on the Fields page).
pub const DOCS: &[(&str, &str)] = &[
    ("origin", "who produced the prompt: typed, slash_command, task_notification, agent_message, subagent, programmatic or unknown"),
    ("typed_text", "the prompt without the context blocks a tool adds before or after it (IDE selection, system reminder)"),
    ("injected", "the context blocks removed from the typed text: their kind (tag) and size in characters"),
];

/// A context block removed from the typed text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Injected {
    /// The block's tag, e.g. `ide_selection`.
    pub kind: String,
    /// Size of the block in characters (not bytes), surrounding whitespace excluded.
    pub chars: usize,
}

/// A prompt's origin, typed text and injected context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classified {
    pub origin: Origin,
    /// `None` when the prompt has no text.
    pub typed_text: Option<String>,
    /// In text order: leading blocks first, then trailing blocks.
    pub injected: Vec<Injected>,
}

/// Remove the `<tag ...>...</tag>` blocks a tool put before or after what the user typed, for any
/// tag in `tags` (longest tag names tried first). Only whole blocks at the very start or end
/// (whitespace around them allowed) are removed, repeatedly; a block in the middle is part of the
/// prompt and stays, an unclosed block stays, a block may not contain another opening of its own
/// tag. Returns the rest, trimmed, and the removed blocks in text order. With no tags the text is
/// only trimmed.
pub fn split_context(_text: &str, _tags: &[String]) -> (String, Vec<Injected>) {
    todo!()
}

/// Whether a rule's conditions hold for a prompt and the events of its turn (origin and surface
/// rules share them). Every condition given must hold:
///
/// * `text_prefix`: `typed` starts with it; `text_regex`: matches at the start of `typed`
/// * `field`: the prompt's mapped field has a value (not null, `""` or `[]`)
/// * without `event`: `path` (when given) of the prompt's raw payload has a value, or equals `equals`
/// * with `event`: some event of the turn with that name satisfies the `path`/`equals` condition
///   (any event of that name when there is no `path`)
pub fn matches(_when: &When, _prompt: &PromptRow, _events: &[EventRow], _typed: &str) -> bool {
    todo!()
}

/// The origin of the first rule that matches, else [`DEFAULT`].
pub fn origin_of(_prompt: &PromptRow, _events: &[EventRow], _rules: &[OriginRule], _typed: &str) -> Origin {
    todo!()
}

/// A stored prompt and the events of its turn -> its origin, typed text and injected context.
///
/// * no text: `unknown`, no typed text
/// * no spec (its source has none any more): `unknown`, the text as typed text, nothing injected
/// * nothing left once the injected blocks are removed: `unknown` (typed text `""`)
/// * else the spec's rules, tried on the typed text (injected blocks removed)
pub fn classify(_prompt: &PromptRow, _events: &[EventRow], _spec: Option<&Spec>) -> Classified {
    todo!()
}
