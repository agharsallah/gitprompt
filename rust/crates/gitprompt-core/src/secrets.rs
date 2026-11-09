//! Finding credentials in text and hiding them in stored JSON. To detect a new kind of secret, add
//! one named pattern to [`KINDS`]. Port of `insights/redaction.py`.

use crate::Json;

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

/// Names of the kinds of secrets found (never the values), in [`KINDS`] order.
pub fn find_secrets(_text: &str) -> Vec<&'static str> {
    todo!()
}

/// Replace every secret match with `[hidden <kind>]`.
pub fn hide(_text: &str) -> String {
    todo!()
}

/// Replace every secret match in a JSON value, keeping its structure.
pub fn redact(_value: &Json) -> Json {
    todo!()
}
