//! What a spec's strings expand to: `${VAR:-default}` and `~` in its paths, and the `{placeholders}`
//! of its `resume` command.

use std::path::PathBuf;

use crate::Settings;
use crate::settings::expand_tilde;

/// `${VAR:-default}` expansion from `settings.env`, then `~` resolved against `settings.user_home`.
pub fn expand(path: &str, settings: &Settings) -> PathBuf {
    expand_tilde(&expand_vars(path, &settings.env), &settings.user_home)
}

/// Every `${VAR:-default}`: the variable's value, or `default` when it is unset or empty. Other text,
/// `${VAR}` included, is kept as written.
fn expand_vars(path: &str, env: &crate::Env) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some((name, default, len)) = parse_var(after) {
            out.push_str(env.get(name).unwrap_or(default));
            rest = &after[len..];
        } else {
            out.push_str("${");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `NAME:-default}` at the start of `text` -> (name, default, length including the `}`).
fn parse_var(text: &str) -> Option<(&str, &str, usize)> {
    let name_len = text.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    let after_name = text[name_len..].strip_prefix(":-")?;
    let default_len = after_name.find('}')?;
    (name_len > 0).then(|| (&text[..name_len], &after_name[..default_len], name_len + 2 + default_len + 1))
}

/// What a `resume.command` may name between braces. `{{` and `}}` write a literal brace.
pub const RESUME_PLACEHOLDERS: &[&str] = &["session_id", "cwd"];

/// Why a command template can't be filled.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fill {
    /// The template is wrong: an unknown placeholder, a stray or unclosed brace.
    Invalid(String),
    /// The template is fine but the session has no value for this placeholder.
    Missing(String),
}

/// `template` with each `{name}` replaced by `value(name)`, shell-quoted ([`shell_quote`]).
pub(super) fn fill(template: &str, value: impl Fn(&str) -> Option<String>) -> Result<String, Fill> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(escaped) = tail.strip_prefix("{{").or_else(|| tail.strip_prefix("}}")) {
            out.push_str(&tail[..1]);
            rest = escaped;
            continue;
        }
        let name = tail.strip_prefix('{').and_then(|inner| inner.split_once('}')).map(|(name, _)| name);
        let Some(name) = name.filter(|name| RESUME_PLACEHOLDERS.contains(name)) else {
            return Err(Fill::Invalid(format!(
                "resume command {template:?} has an unknown placeholder or a stray brace (placeholders: {RESUME_PLACEHOLDERS:?})"
            )));
        };
        out.push_str(&shell_quote(&value(name).ok_or_else(|| Fill::Missing(name.to_owned()))?));
        rest = &tail[name.len() + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// `value` as one POSIX shell word: unchanged when it has only characters the shell never treats
/// specially, else in single quotes with each `'` written as `'\''`. The empty string is `''`.
pub fn shell_quote(value: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '=' | ',');
    if !value.is_empty() && value.chars().all(plain) {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// A spec's `resume.command` for one session: `None` when the template needs a value the session
/// doesn't have (`cwd`) or has none, never a command with a hole in it.
pub(super) fn resume_command(template: &str, session_id: &str, cwd: Option<&str>) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }
    let value = |name: &str| match name {
        "session_id" => Some(session_id.to_owned()),
        "cwd" => cwd.filter(|cwd| !cwd.is_empty()).map(str::to_owned),
        _ => None,
    };
    fill(template, value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vars_without_a_default_or_unterminated_are_kept() {
        let env = crate::Env::from_pairs([("A", "x")]);
        assert_eq!(expand_vars("${A}/${A:-d}/${:-e}/${A:-", &env), "${A}/x/${:-e}/${A:-");
    }
}
