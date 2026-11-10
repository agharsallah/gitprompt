//! Hook commands that run GitPrompt's forwarder instead of curl (see the [`super`] module docs).

use std::path::Path;

use gitprompt_core::{Error, Json, Result};

use crate::spool::is_plain_name;

/// What a bundled curl forwarder posts to: GitPrompt's receiver for this source, unrendered.
const INGEST_URL: &str = "{{receiver}}/ingest/{{source}}";

/// `'<forwarder>' hook <source> # /ingest/<source>`: runs the forwarder for `source`; the comment
/// carries the marker ([`gitprompt_core::spec::Spec::marker`]). `Error::Invalid` for a source id that
/// isn't a plain name (it is written unquoted, and in a comment) or a forwarder path that isn't
/// valid UTF-8.
pub(super) fn forwarder_command(forwarder: &Path, source: &str) -> Result<String> {
    if !is_plain_name(source) {
        return Err(Error::Invalid(format!("{source:?} can't be put in a hook command: a source id must be a-z, 0-9 and -")));
    }
    let path =
        forwarder.to_str().ok_or_else(|| Error::Invalid(format!("the forwarder path {} is not valid UTF-8", forwarder.display())))?;
    Ok(format!("{} hook {source} # /ingest/{source}", shell_quote(path)))
}

/// `text` as one POSIX shell word: in single quotes, each `'` written as `'\''`.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A bundled curl forwarder command (before rendering): it starts with `curl ` and posts to
/// [`INGEST_URL`].
fn is_curl_forwarder(text: &str) -> bool {
    text.trim_start().starts_with("curl ") && text.contains(INGEST_URL)
}

/// The template with every curl forwarder command replaced by `command`; everything else as is.
pub(super) fn replace_curl_commands(template: &Json, command: &str) -> Json {
    gitprompt_core::json::map_strings(template, &|text| if is_curl_forwarder(text) { command.to_owned() } else { text.to_owned() })
}

/// How GitPrompt's hook commands for `source` are installed in a config file's `text`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "form", rename_all = "lowercase")]
pub enum HookForm {
    /// The spec's curl commands.
    Curl,
    /// GitPrompt's forwarder ([`forwarder_command`]); `path` is the executable the commands run,
    /// `None` when it can't be read back.
    Forwarder { path: Option<String> },
}

/// The form of the installed commands: [`HookForm::Forwarder`] when `text` holds a forwarder
/// command for `source`, else [`HookForm::Curl`]. (Whether anything is installed at all is
/// `Connection::connected`.)
pub(super) fn installed_form(text: &str, source: &str) -> HookForm {
    let tail = format!(" hook {source} # /ingest/{source}");
    let Some(end) = text.find(&tail) else { return HookForm::Curl };
    // The command is the end of a JSON (or TOML) string: it starts after the last unescaped quote.
    let before = &text[..end];
    let start = before.char_indices().rev().find(|&(i, c)| c == '"' && !before[..i].ends_with('\\')).map_or(0, |(i, _)| i + 1);
    let word = before[start..].trim().replace("\\\\", "\\").replace("\\/", "/").replace("\\\"", "\"");
    HookForm::Forwarder { path: shell_unquote(&word) }
}

/// The value of one POSIX shell word made of `'...'` parts, `\x` escapes and plain characters;
/// `None` for an unterminated quote or an empty word.
fn shell_unquote(word: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = word.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => loop {
                match chars.next()? {
                    '\'' => break,
                    inner => out.push(inner),
                }
            },
            '\\' => out.push(chars.next()?),
            c if c.is_whitespace() => return None, // more than one word: not ours
            c => out.push(c),
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_installed_form_is_read_back_from_the_config_text() {
        let path = "/Apps/Git Prompt's.app/gitprompt";
        let command = forwarder_command(Path::new(path), "cursor").unwrap();
        let text = serde_json::to_string_pretty(&json!({"hooks": {"x": [{"command": command}]}})).unwrap();
        assert_eq!(installed_form(&text, "cursor"), HookForm::Forwarder { path: Some(path.into()) });
        assert_eq!(installed_form(&text, "codex"), HookForm::Curl);
        let curl = r#"{"command": "curl -s --data-binary @- 'http://127.0.0.1:7878/ingest/cursor'"}"#;
        assert_eq!(installed_form(curl, "cursor"), HookForm::Curl);
    }

    #[test]
    fn a_forwarder_path_that_cannot_be_read_back_is_none() {
        let text = r#"{"command": "'/unterminated hook cursor # /ingest/cursor"}"#;
        assert_eq!(installed_form(text, "cursor"), HookForm::Forwarder { path: None });
    }

    #[test]
    fn a_path_with_spaces_and_quotes_stays_one_shell_word() {
        let command = forwarder_command(Path::new("/Apps/Git Prompt's.app/gitprompt"), "cursor").unwrap();
        assert_eq!(command, r"'/Apps/Git Prompt'\''s.app/gitprompt' hook cursor # /ingest/cursor");
    }

    #[test]
    fn a_source_id_that_is_not_a_plain_name_is_refused() {
        let err = forwarder_command(Path::new("/bin/gitprompt"), "x\nrm -rf ~").unwrap_err();
        assert!(matches!(err, Error::Invalid(_)), "{err:?}");
    }

    #[test]
    fn only_curl_commands_posting_to_the_receiver_are_replaced() {
        let template = json!({
            "hook": "curl -s --data-binary @- '{{receiver}}/ingest/{{source}}' >/dev/null 2>&1 || true",
            "endpoint": "{{receiver}}/ingest/{{source}}",
            "other": "curl https://example.com",
        });
        let replaced = replace_curl_commands(&template, "FORWARD");
        assert_eq!(replaced, json!({"hook": "FORWARD", "endpoint": "{{receiver}}/ingest/{{source}}", "other": "curl https://example.com"}));
    }
}
