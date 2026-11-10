//! What the prompt text itself says ([`PromptStats`]): size, references, credentials.

use gitprompt_core::secrets::find_secrets;

use super::PromptStats;
use crate::text::{is_space, is_word, words};

/// What each value means, by name under `stats.` (the Fields page).
pub(super) const DOCS: &[(&str, &str)] = &[
    ("chars / words / lines / approx_tokens", "prompt size (tokens ≈ chars / 4)"),
    ("slash_command", "command the prompt starts with, e.g. /review"),
    ("at_mentions / urls / code_blocks / questions", "what the prompt references"),
    ("secrets", "kinds of credentials found in the prompt (values never shown)"),
];

pub(super) fn prompt_stats(text: &str) -> PromptStats {
    let chars = text.chars().count();
    PromptStats {
        chars,
        words: words(text).count(),
        lines: if text.is_empty() { 0 } else { text.matches('\n').count() + 1 },
        approx_tokens: chars.div_ceil(4),
        slash_command: slash_command(text),
        at_mentions: at_mentions(text),
        urls: urls(text),
        code_blocks: text.matches("```").count() / 2,
        questions: text.matches('?').count(),
        secrets: find_secrets(text).into_iter().map(String::from).collect(),
    }
}

/// A character of a command name: a word character, `:` or `-`.
fn is_command(c: char) -> bool {
    is_word(c) || c == ':' || c == '-'
}

/// A character of an `@` mention: a word character, `.`, `/`, `~` or `-`.
fn is_mention(c: char) -> bool {
    is_word(c) || matches!(c, '.' | '/' | '~' | '-')
}

/// `/name` at the start of the text, after optional whitespace.
fn slash_command(text: &str) -> Option<String> {
    let rest = text.trim_start_matches(is_space).strip_prefix('/')?;
    let name = &rest[..rest.find(|c| !is_command(c)).unwrap_or(rest.len())];
    (!name.is_empty()).then(|| format!("/{name}"))
}

/// `@path` mentions preceded by whitespace or the start of the text.
fn at_mentions(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut previous: Option<char> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '@' && previous.is_none_or(is_space) {
            let rest = &text[at + 1..];
            let name = &rest[..rest.find(|c| !is_mention(c)).unwrap_or(rest.len())];
            if !name.is_empty() {
                out.push(name.to_owned());
                // Continue after the mention; its last character is the one before what follows.
                while chars.next_if(|(i, _)| *i < at + 1 + name.len()).is_some() {}
                previous = name.chars().next_back();
                continue;
            }
        }
        previous = Some(c);
    }
    out
}

/// A character that ends a URL: whitespace, `)`, `>` or `]`.
fn ends_url(c: char) -> bool {
    is_space(c) || matches!(c, ')' | '>' | ']')
}

/// `http(s)://...` up to a character that ends a URL.
fn urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("http") {
        let start = from + found;
        let rest = &text[start..];
        let scheme = ["https://", "http://"].into_iter().find(|scheme| rest.starts_with(scheme)).map_or(0, str::len);
        let tail = &rest[scheme..];
        let len = tail.find(ends_url).unwrap_or(tail.len());
        if scheme > 0 && len > 0 {
            out.push(rest[..scheme + len].to_owned());
            from = start + scheme + len;
        } else {
            from = start + "http".len();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_need_whitespace_before_and_stop_at_other_characters() {
        assert_eq!(at_mentions("@a b@c @d,@e"), vec!["a", "d"]);
    }

    #[test]
    fn a_url_needs_a_character_after_its_scheme() {
        assert_eq!(urls("http:// https://x) httpss://y"), vec!["https://x"]);
    }
}
