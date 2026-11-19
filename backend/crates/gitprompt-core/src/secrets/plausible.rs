//! Which secret matches are worth a finding. A pattern finds the shape of a key; this module turns
//! away what only has the shape: placeholders and published examples, code that names a secret
//! without holding it (`api_key = os.environ["KEY"]`), and values too regular to be random.

use crate::leak::{Found, Mask, Scope, entropy, is_placeholder};

/// A random key has at least this entropy per character; words and repeated characters have less.
pub(crate) const TOKEN_ENTROPY: f64 = 3.0;
/// A password is allowed to be weaker than a key, but not a word repeated.
pub(crate) const PASSWORD_ENTROPY: f64 = 2.5;
/// Shortest password worth a finding.
const PASSWORD_CHARS: usize = 8;
/// Kinds of character (letters, digits, others) a password mixes at least: a word alone is not one.
const PASSWORD_CLASSES: usize = 2;
/// Base64 characters a private key block holds at least (a header alone is documentation).
const KEY_BODY_CHARS: usize = 64;

/// The jwt.io sample token's signature: in every tutorial, never anyone's.
const JWT_SAMPLE: &str = "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

/// Characters that make a value code rather than a literal.
const CODE_CHARS: &[char] = &['(', ')', '[', ']', '{', '}', '$', '<', '>', '%', '`', '\\', ',', ';', '|'];
/// How code reads a secret it does not hold.
const CODE_PREFIXES: &[&str] =
    &["process.", "os.", "env.", "self.", "this.", "config.", "settings.", "getenv", "secrets.", "vault:", "op://"];

/// The finding for a `kind` match, if the match is plausibly a real secret.
pub(super) fn judge<'t>(kind: &'static str, matched: &'t str) -> Option<Found<'t>> {
    let (value, mask) = match kind {
        "private key" => (has_key_body(matched).then_some(matched)?, Mask::Hidden),
        "password or key assignment" => (password(after_any(matched, &[':', '='])?)?, Mask::Token),
        "credentials in a URL" => (password(url_password(matched)?)?, Mask::Token),
        "bearer token" => (token(after_bearer(matched)?)?, Mask::Token),
        "JWT" => (token(matched).filter(|t| !t.ends_with(JWT_SAMPLE))?, Mask::Token),
        _ => (token(matched)?, Mask::Token),
    };
    Some(Found { kind, value, mask, scope: Scope::Anywhere })
}

/// A key: random enough, and not an example.
fn token(value: &str) -> Option<&str> {
    (!is_placeholder(value) && entropy(value) >= TOKEN_ENTROPY).then_some(value)
}

/// A literal password: long enough, not code, not a name or a word, not an example.
fn password(value: &str) -> Option<&str> {
    // In a form or query string the next field starts at `&name=`.
    let value = match value.split_once('&') {
        Some((head, rest)) if rest.contains('=') => head,
        _ => value,
    };
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim_end_matches([':', '.', ',', ';', '!', '?']);
    let plausible = value.chars().count() >= PASSWORD_CHARS
        && !is_code(value)
        && !is_name(value)
        && classes(value) >= PASSWORD_CLASSES
        && !is_placeholder(value)
        && entropy(value) >= PASSWORD_ENTROPY;
    plausible.then_some(value)
}

/// Code that reads a secret, or a URL, an address, a path or the next assignment in its place.
fn is_code(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let elsewhere = ["://", "@", "="].iter().any(|mark| value.contains(mark));
    let path = ["/", "~", "./", "../"].iter().any(|start| value.starts_with(start));
    value.contains(CODE_CHARS) || elsewhere || path || CODE_PREFIXES.iter().any(|prefix| lower.starts_with(prefix))
}

/// An identifier: `ENV_VAR_NAME`, `snake_case`, `kebab-case`, or a dotted path (`cfg.sessionSecret`).
fn is_name(value: &str) -> bool {
    let env = value.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    let lower = value.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '.' || c == '-');
    let identifier = |part: &str| {
        part.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    let path = value.contains('.') && value.split('.').all(identifier);
    env || lower || path
}

/// How many of letters, digits and other characters the value mixes: a real password has two.
fn classes(value: &str) -> usize {
    let has = |test: fn(&char) -> bool| usize::from(value.chars().any(|c| test(&c)));
    has(char::is_ascii_alphabetic) + has(char::is_ascii_digit) + has(|c| !c.is_ascii_alphanumeric())
}

/// What follows the first of `separators`.
fn after_any<'t>(text: &'t str, separators: &[char]) -> Option<&'t str> {
    text.split_once(separators).map(|(_, value)| value)
}

/// The token after `Bearer`.
fn after_bearer(text: &str) -> Option<&str> {
    let at = text.to_ascii_lowercase().rfind("bearer")?;
    text.get(at + "bearer".len()..).map(str::trim)
}

/// `scheme://user:password@host` → `password`.
fn url_password(text: &str) -> Option<&str> {
    let (_, rest) = text.split_once("://")?;
    let (credentials, _) = rest.rsplit_once('@')?;
    credentials.split_once(':').map(|(_, password)| password)
}

/// The block holds key material, not only its header and footer.
fn has_key_body(block: &str) -> bool {
    let body = block.lines().filter(|line| !line.starts_with("-----")).flat_map(str::chars);
    body.filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')).count() >= KEY_BODY_CHARS
}
