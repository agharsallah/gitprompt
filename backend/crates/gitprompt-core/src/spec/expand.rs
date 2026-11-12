//! `${VAR:-default}` and `~` in the paths a spec names.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vars_without_a_default_or_unterminated_are_kept() {
        let env = crate::Env::from_pairs([("A", "x")]);
        assert_eq!(expand_vars("${A}/${A:-d}/${:-e}/${A:-", &env), "${A}/x/${:-e}/${A:-");
    }
}
