//! Finding credentials in text and hiding them in stored JSON. To detect a new kind of secret, add
//! one named pattern to [`KINDS`]. Port of `insights/redaction.py`.

use std::borrow::Cow;
use std::sync::LazyLock;

use fancy_regex::{NoExpand, Regex};

use crate::Json;
use crate::json::map_strings;
use crate::leak::Found;

mod plausible;

/// (name, pattern) of every kind of secret detected, in detection order.
pub const KINDS: &[(&str, &str)] = &[
    ("private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)"),
    ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_\w{22,})"),
    ("Anthropic API key", r"\bsk-ant-[\w-]{20,}"),
    // "not sk-ant-" spelled without a lookahead, so the pattern runs on the fast engine.
    (
        "OpenAI API key",
        r"\bsk-(?:proj-[\w-]{20,}|[[\w-]&&[^a]][\w-]{19,}|a[[\w-]&&[^n]][\w-]{18,}|an[[\w-]&&[^t]][\w-]{17,}|ant[[\w-]&&[^-]][\w-]{16,})",
    ),
    ("Slack token", r"\bxox[abposr]-[\w-]{10,}"),
    ("Google API key", r"\bAIza[\w-]{35}"),
    ("Stripe key", r"\b[rs]k_live_[A-Za-z0-9]{20,}"),
    ("JWT", r"\beyJ[\w-]{10,}\.[\w-]{10,}\.[\w-]{10,}"),
    ("GitLab token", r"\bglpat-[\w-]{20,}"),
    ("npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    ("PyPI token", r"\bpypi-AgEIcHlwaS5vcmc[\w-]{50,}"),
    ("Hugging Face token", r"\bhf_[A-Za-z0-9]{34,}\b"),
    ("SendGrid key", r"\bSG\.[\w-]{22}\.[\w-]{43}\b"),
    ("Atlassian token", r"\bATATT3[A-Za-z0-9_=-]{50,}"),
    ("credentials in a URL", r"\b[a-z][a-z0-9+.-]{1,20}://[^\s:/@]{1,64}:[^\s:/@]{3,128}@[\w.-]+"),
    ("bearer token", r"(?i)\bauthorization:\s*bearer\s+[\w.~+/=-]{20,}"),
    ("password or key assignment", r#"(?i)\b[\w-]*?(?:password|passwd|pwd|secret|api[_-]?key|token)[ \t]*[:=][ \t]*['"]?[^\s'"]{8,}"#),
];

/// [`KINDS`] compiled once, with each kind's replacement text.
static COMPILED: LazyLock<Vec<Kind>> = LazyLock::new(|| KINDS.iter().map(|&(name, pattern)| Kind::new(name, pattern)).collect());

struct Kind {
    name: &'static str,
    regex: Regex,
    replacement: String,
}

impl Kind {
    #[expect(clippy::expect_used, reason = "KINDS are constants, each compiled by `every_kind_compiles`")]
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

/// Every secret in `text` that is plausibly real, with its value: placeholders, examples, code
/// that only names a secret and values too regular to be a key are left out (unlike [`hide`],
/// which hides every match). A value an earlier kind already found is not found again (the
/// specific kinds come first, `password or key assignment` last).
pub fn find_leaks(text: &str) -> Vec<Found<'_>> {
    let mut found: Vec<Found<'_>> = Vec::new();
    for kind in COMPILED.iter() {
        for m in kind.regex.find_iter(text).filter_map(Result::ok) {
            let Some(leak) = plausible::judge(kind.name, m.as_str()) else { continue };
            if !found.iter().any(|seen| overlaps(text, seen.value, leak.value)) {
                found.push(leak);
            }
        }
    }
    found
}

/// Whether two slices of `text` share a byte.
fn overlaps(text: &str, a: &str, b: &str) -> bool {
    let span = |s: &str| {
        let start = (s.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);
        start..start + s.len()
    };
    let (a, b) = (span(a), span(b));
    a.start < b.end && b.start < a.end
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
