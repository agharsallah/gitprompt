//! Text and number helpers that behave like the Python built-ins the derivations were written with
//! (`str.isspace`, `str.split`, `round`, `float`), so derived values stay the same after the port.

use gitprompt_core::Json;

/// Python's `str.isspace` for one character: Unicode whitespace plus the ASCII file, group, record
/// and unit separators (`\x1c`..`\x1f`), which Python counts as whitespace and Rust does not.
pub(crate) fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// Python's `\w` for one character: a letter, a digit or `_`.
pub(crate) fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Python's `str.strip()`.
pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// Python's `str.split()`: runs of whitespace separate words, none are empty.
pub(crate) fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(is_space).filter(|w| !w.is_empty())
}

/// Python's `round(x, digits)` (see `gitprompt_core::numbers::round`).
pub(crate) use gitprompt_core::numbers::round;

/// Python's `float(value)` for a payload value, with 0 for anything it would reject: numbers as they
/// are, numeric strings parsed (surrounding whitespace allowed), booleans as 1 and 0.
pub(crate) fn number(value: Option<&Json>) -> f64 {
    match value {
        Some(Json::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Json::String(s)) => trim(s).parse().unwrap_or(0.0),
        Some(Json::Bool(b)) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// Python truthiness of a payload value: null, `false`, `0`, `""`, `[]` and `{}` are false.
pub(crate) fn truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Json::String(s) => !s.is_empty(),
        Json::Array(items) => !items.is_empty(),
        Json::Object(map) => !map.is_empty(),
    }
}

/// A payload value where text is expected: a string as it is, null as `None`, anything else (a
/// number, an object) as its JSON text, so nothing a tool reported is dropped.
pub(crate) fn text_of(value: &Json) -> Option<String> {
    match value {
        Json::Null => None,
        Json::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// `text` cut to `chars` characters, trailing whitespace removed and `…` added when it was longer;
/// whether it was cut.
pub(crate) fn clip(text: &str, chars: usize) -> (String, bool) {
    match text.char_indices().nth(chars) {
        None => (text.to_owned(), false),
        Some((cut, _)) => (format!("{}…", text[..cut].trim_end_matches(is_space)), true),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "rounding tests: the exact float is the expected value")]
mod tests {
    use super::*;

    #[test]
    fn rounding_matches_python_on_exact_ties_and_near_ties() {
        assert_eq!([round(1.25, 1), round(0.15, 1), round(2.675, 2), round(0.000_05, 4)], [1.2, 0.1, 2.67, 0.0001]);
    }

    #[test]
    fn clipping_counts_characters_and_trims_before_the_ellipsis() {
        assert_eq!(clip("ab  cd", 4), ("ab…".to_string(), true));
        assert_eq!(clip("héllo", 5), ("héllo".to_string(), false));
    }
}
