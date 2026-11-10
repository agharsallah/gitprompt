//! Who produced a prompt, and which part of it the user actually typed. Derived on read from the
//! source spec's `origin` rules and `injected` tags (ADR-0010), so a changed rule reclassifies old
//! prompts too, with no migration. Pure functions: no I/O, no knowledge of any tool (that lives in
//! the YAML specs).

use serde::{Deserialize, Serialize};

use gitprompt_core::Json;
use gitprompt_core::json::is_no_value;
use gitprompt_core::spec::{Origin, OriginRule, Spec, When};
use gitprompt_store::{EventRow, PromptRow};

use crate::raw::{self, Event, Raw};
use crate::text::{is_space, trim};

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
pub fn split_context(text: &str, tags: &[String]) -> (String, Vec<Injected>) {
    if tags.is_empty() {
        return (trim(text).to_owned(), Vec::new());
    }
    let mut tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    tags.sort_by_key(|tag| std::cmp::Reverse(tag.len()));
    let mut rest = text;
    let mut leading = Vec::new();
    while let Some((tag, block)) = leading_block(rest, &tags) {
        leading.push(injected(tag, &rest[block.clone()]));
        rest = rest[block.end..].trim_start_matches(is_space);
    }
    let mut trailing = Vec::new();
    while let Some((tag, block)) = trailing_block(rest, &tags) {
        trailing.push(injected(tag, &rest[block.clone()]));
        rest = rest[..block.start].trim_end_matches(is_space);
    }
    leading.extend(trailing.into_iter().rev());
    (trim(rest).to_owned(), leading)
}

/// Whether `text` opens (after whitespace) with a whole `<tag ...>...</tag>` block of one of `tags`.
pub fn opens_with_block(text: &str, tags: &[String]) -> bool {
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    leading_block(text, &tags).is_some()
}

fn injected(tag: &str, block: &str) -> Injected {
    Injected { kind: tag.to_owned(), chars: block.chars().count() }
}

/// The tag and byte range of a block that starts `text` (after whitespace).
fn leading_block<'t>(text: &str, tags: &[&'t str]) -> Option<(&'t str, std::ops::Range<usize>)> {
    let start = text.len() - text.trim_start_matches(is_space).len();
    tags.iter().find_map(|tag| {
        let body = opening_end(text, start, tag)?;
        let close = closing(text, body, tag, |_| true)?;
        Some((*tag, start..close + closing_tag(tag).len()))
    })
}

/// The tag and byte range of a block that ends `text` (before whitespace): the leftmost opening
/// whose block runs to the end.
fn trailing_block<'t>(text: &str, tags: &[&'t str]) -> Option<(&'t str, std::ops::Range<usize>)> {
    let end = text.trim_end_matches(is_space).len();
    let mut found: Option<(&'t str, std::ops::Range<usize>)> = None;
    for tag in tags {
        let close_tag = closing_tag(tag);
        let at_end = |close: usize| close + close_tag.len() == end;
        let opening = format!("<{tag}");
        for (start, _) in text[..end].match_indices(&opening) {
            if found.as_ref().is_some_and(|(_, block)| block.start <= start) {
                break;
            }
            let Some(body) = opening_end(text, start, tag) else { continue };
            if closing(text, body, tag, at_end).is_some() {
                found = Some((*tag, start..end));
                break;
            }
        }
    }
    found
}

fn closing_tag(tag: &str) -> String {
    format!("</{tag}>")
}

/// Where the body of a `<tag>` or `<tag attributes>` opening at `start` begins.
fn opening_end(text: &str, start: usize, tag: &str) -> Option<usize> {
    let after = text[start..].strip_prefix('<')?.strip_prefix(tag)?;
    let offset = start + 1 + tag.len();
    match after.chars().next()? {
        '>' => Some(offset + 1),
        c if is_space(c) => after.find('>').map(|gt| offset + gt + 1),
        _ => None,
    }
}

/// The position of the first `</tag>` from `body` that `accept` takes (the block's end), as a lazy
/// regex would find it: `None` once the body reaches another opening of the same tag.
fn closing(text: &str, body: usize, tag: &str, accept: impl Fn(usize) -> bool) -> Option<usize> {
    let close_tag = closing_tag(tag);
    let opening = format!("<{tag}");
    for (at, _) in text[body..].char_indices() {
        let here = &text[body + at..];
        if here.starts_with(&close_tag) && accept(body + at) {
            return Some(body + at);
        }
        if let Some(next) = here.strip_prefix(&opening).and_then(|rest| rest.chars().next())
            && (next == '>' || is_space(next))
        {
            return None;
        }
    }
    None
}

/// Whether a rule's conditions hold for a prompt and the events of its turn (origin and surface
/// rules share them). Every condition given must hold:
///
/// * `text_prefix`: `typed` starts with it; `text_regex`: matches at the start of `typed`
/// * `text_block`: `typed` opens with a whole block of one of its tags, as [`split_context`] reads
///   blocks (so `<tag-more>`, an unclosed block or one after other text don't count)
/// * `field`: the prompt's mapped field has a value (not null, `""` or `[]`)
/// * without `event`: `path` (when given) of the prompt's raw payload has a value, or equals `equals`
/// * with `event`: some event of the turn with that name satisfies the `path`/`equals` condition
///   (any event of that name when there is no `path`)
pub fn matches(when: &When, prompt: &PromptRow, events: &[EventRow], typed: &str) -> bool {
    matches_in(when, prompt, &Raw::new(&prompt.raw, None), &raw::events(events, None), typed)
}

/// [`matches()`] over payloads being read (and maybe recorded).
pub(crate) fn matches_in(when: &When, prompt: &PromptRow, raw: &Raw<'_>, events: &[Event<'_>], typed: &str) -> bool {
    if when.text_prefix.as_ref().is_some_and(|prefix| !typed.starts_with(prefix.as_str())) {
        return false;
    }
    if when.text_regex.as_ref().is_some_and(|regex| !regex.matches_start(typed)) {
        return false;
    }
    if when.text_block.as_ref().is_some_and(|tags| !opens_with_block(typed, tags)) {
        return false;
    }
    if when.field.as_ref().is_some_and(|field| prompt.fields.get(field).is_none_or(is_no_value)) {
        return false;
    }
    match &when.event {
        None => value_matches(raw, when),
        Some(name) => events.iter().any(|e| e.name() == Some(name.as_str()) && value_matches(&e.raw, when)),
    }
}

/// The rule's `path`/`equals` condition on one payload.
fn value_matches(payload: &Raw<'_>, when: &When) -> bool {
    let Some(path) = &when.path else { return true };
    let value = payload.pluck(path);
    match &when.equals {
        None => value.is_some_and(|v| !is_no_value(v.json())),
        Some(expected) => value.is_some_and(|v| same_json(v.json(), expected)),
    }
}

/// JSON equality as Python compares parsed JSON: numbers by value (`1` = `1.0`), recursively.
pub(crate) fn same_json(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (Json::Array(x), Json::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_json(a, b)),
        (Json::Object(x), Json::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w))),
        _ => a == b,
    }
}

/// The origin of the first rule that matches, else [`DEFAULT`].
pub fn origin_of(prompt: &PromptRow, events: &[EventRow], rules: &[OriginRule], typed: &str) -> Origin {
    origin_in(prompt, &Raw::new(&prompt.raw, None), &raw::events(events, None), rules, typed)
}

fn origin_in(prompt: &PromptRow, raw: &Raw<'_>, events: &[Event<'_>], rules: &[OriginRule], typed: &str) -> Origin {
    rules.iter().find(|rule| matches_in(&rule.when, prompt, raw, events, typed)).map_or(DEFAULT, |rule| rule.origin)
}

/// A stored prompt and the events of its turn -> its origin, typed text and injected context.
///
/// * no text: `unknown`, no typed text
/// * no spec (its source has none any more): `unknown`, the text as typed text, nothing injected
/// * nothing left once the injected blocks are removed: `unknown` (typed text `""`)
/// * else the spec's rules, tried on the typed text (injected blocks removed)
pub fn classify(prompt: &PromptRow, events: &[EventRow], spec: Option<&Spec>) -> Classified {
    classify_in(prompt, &Raw::new(&prompt.raw, None), &raw::events(events, None), spec)
}

/// [`classify`] over payloads being read (and maybe recorded).
pub(crate) fn classify_in(prompt: &PromptRow, raw: &Raw<'_>, events: &[Event<'_>], spec: Option<&Spec>) -> Classified {
    let Some(text) = &prompt.prompt else {
        return Classified { origin: Origin::Unknown, typed_text: None, injected: Vec::new() };
    };
    let Some(spec) = spec else {
        return Classified { origin: Origin::Unknown, typed_text: Some(text.clone()), injected: Vec::new() };
    };
    let (typed, injected) = split_context(text, &spec.injected);
    let origin = if typed.is_empty() { Origin::Unknown } else { origin_in(prompt, raw, events, &spec.origin, &typed) };
    Classified { origin, typed_text: Some(typed), injected }
}

/// The name of an origin as the API and the prompt index write it (`typed`, `slash_command`...).
pub fn name(origin: Origin) -> &'static str {
    match origin {
        Origin::Typed => "typed",
        Origin::SlashCommand => "slash_command",
        Origin::TaskNotification => "task_notification",
        Origin::AgentMessage => "agent_message",
        Origin::Subagent => "subagent",
        Origin::Programmatic => "programmatic",
        Origin::Unknown => "unknown",
    }
}

/// The origin [`name`]d `text`; `None` for any other text.
pub fn parse(text: &str) -> Option<Origin> {
    [
        Origin::Typed,
        Origin::SlashCommand,
        Origin::TaskNotification,
        Origin::AgentMessage,
        Origin::Subagent,
        Origin::Programmatic,
        Origin::Unknown,
    ]
    .into_iter()
    .find(|origin| name(*origin) == text)
}
