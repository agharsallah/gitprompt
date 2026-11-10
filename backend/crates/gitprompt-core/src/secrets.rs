//! Finding credentials in text and hiding them in stored JSON. To detect a new kind of secret, add
//! one named pattern to [`KINDS`]. Port of `insights/redaction.py`.

use std::borrow::Cow;
use std::sync::LazyLock;

use fancy_regex::{NoExpand, Regex};

use crate::Json;
use crate::json::map_strings;

/// (name, pattern) of every kind of secret detected, in detection order.
pub const KINDS: &[(&str, &str)] = &[
    ("private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)"),
    ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_\w{22,})"),
    ("Anthropic API key", r"\bsk-ant-[\w-]{20,}"),
    ("OpenAI API key", r"\bsk-(?!ant-)(?:proj-)?[\w-]{20,}"),
    ("Slack token", r"\bxox[abposr]-[\w-]{10,}"),
    ("Google API key", r"\bAIza[\w-]{35}"),
    ("Stripe key", r"\b[rs]k_live_[A-Za-z0-9]{20,}"),
    ("JWT", r"\beyJ[\w-]{10,}\.[\w-]{10,}\.[\w-]{10,}"),
    ("password or key assignment", r#"(?i)\b(?:password|passwd|pwd|secret|api[_-]?key|token)\s*[:=]\s*['"]?[^\s'"]{8,}"#),
];

/// [`KINDS`] compiled once, with each kind's replacement text.
static COMPILED: LazyLock<Vec<Kind>> = LazyLock::new(|| KINDS.iter().map(|&(name, pattern)| Kind::new(name, pattern)).collect());

struct Kind {
    name: &'static str,
    regex: Regex,
    replacement: String,
}

impl Kind {
    fn new(name: &'static str, pattern: &str) -> Self {
        // KINDS are constants and every one is compiled by the tests (`every_kind_compiles`), so this
        // cannot fail at run time.
        let regex = Regex::new(pattern).expect("secret patterns in KINDS are valid");
        Kind { name, regex, replacement: format!("[hidden {name}]") }
    }

    /// A regex run-time error (backtrack limit) is reported as a match: better to flag or hide a
    /// harmless text than to let a secret through.
    fn found_in(&self, text: &str) -> bool {
        self.regex.is_match(text).unwrap_or_else(|e| {
            tracing::warn!(kind = self.name, error = %e, "secret pattern failed; treating the text as containing one");
            true
        })
    }

    fn hide<'t>(&self, text: &'t str) -> Cow<'t, str> {
        self.regex.try_replacen(text, 0, NoExpand(&self.replacement)).unwrap_or_else(|e| {
            tracing::warn!(kind = self.name, error = %e, "secret pattern failed; hiding the whole text");
            Cow::Owned(self.replacement.clone())
        })
    }
}

/// Names of the kinds of secrets found (never the values), in [`KINDS`] order.
pub fn find_secrets(text: &str) -> Vec<&'static str> {
    COMPILED.iter().filter(|kind| kind.found_in(text)).map(|kind| kind.name).collect()
}

/// Replace every secret match with `[hidden <kind>]`.
pub fn hide(text: &str) -> String {
    COMPILED.iter().fold(text.to_owned(), |text, kind| kind.hide(&text).into_owned())
}

/// Replace every secret match in a JSON value, keeping its structure.
pub fn redact(value: &Json) -> Json {
    map_strings(value, &hide)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_compiles() {
        for (name, pattern) in KINDS {
            assert!(Regex::new(pattern).is_ok(), "{name}");
        }
        assert_eq!(COMPILED.len(), KINDS.len());
    }
}
