//! A spec's `resume` command: validation of its placeholders, filling them in, shell quoting, and
//! the bundled specs' templates.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use common::{MINIMAL, bundled};
use gitprompt_core::spec::{RESUME_PLACEHOLDERS, Spec, bundled_defaults, parse_spec, shell_quote};
use gitprompt_core::{Error, Result};
use pretty_assertions::assert_eq;

fn parse(extra: &str) -> Result<Spec> {
    parse_spec("mytool.yaml", &format!("{MINIMAL}{extra}\n"), &serde_norway::Mapping::new())
}

fn with_command(command: &str) -> Spec {
    parse(&format!("resume: {{command: \"{command}\"}}")).unwrap()
}

fn bundled_spec(id: &str) -> Spec {
    parse_spec(&format!("{id}.yaml"), bundled(&format!("{id}.yaml")), &bundled_defaults()).unwrap()
}

#[test]
fn a_spec_without_resume_has_no_command() {
    let spec = parse("").unwrap();
    assert_eq!(spec.resume, None);
    assert_eq!(spec.resume_command("abc", Some("/work")), None);
}

#[test]
fn the_session_id_is_filled_in() {
    assert_eq!(with_command("mytool --resume {session_id}").resume_command("1b2c-3d", None).as_deref(), Some("mytool --resume 1b2c-3d"));
}

#[test]
fn the_folder_is_filled_in_when_the_session_has_one() {
    let spec = with_command("mytool --dir {cwd} --resume {session_id}");
    assert_eq!(spec.resume_command("s1", Some("/work/app")).as_deref(), Some("mytool --dir /work/app --resume s1"));
    assert_eq!(spec.resume_command("s1", None), None, "never a command with a hole in it");
    assert_eq!(spec.resume_command("s1", Some("")), None);
}

#[test]
fn a_session_without_an_id_has_no_command() {
    assert_eq!(with_command("mytool --resume {session_id}").resume_command("", None), None);
}

#[test]
fn doubled_braces_write_a_brace() {
    assert_eq!(with_command("mytool '{{x}}' {session_id}").resume_command("s", None).as_deref(), Some("mytool '{x}' s"));
}

#[test]
fn values_with_spaces_quotes_or_shell_syntax_come_out_as_one_safe_word() {
    let spec = with_command("mytool --dir {cwd} --resume {session_id}");
    let command = spec.resume_command("a b", Some("/Users/me/My Project")).unwrap();
    assert_eq!(command, "mytool --dir '/Users/me/My Project' --resume 'a b'");
    let command = spec.resume_command("x", Some("/work/it's; rm -rf ~ $(id) `id` \"q\"")).unwrap();
    assert_eq!(command, r#"mytool --dir '/work/it'\''s; rm -rf ~ $(id) `id` "q"' --resume x"#);
}

#[test]
fn shell_quote_leaves_plain_words_and_quotes_the_rest() {
    let cases = [
        ("", "''"),
        ("abc-1_2.3/4:5@6%7+8=9,0", "abc-1_2.3/4:5@6%7+8=9,0"),
        ("a b", "'a b'"),
        ("it's", r"'it'\''s'"),
        ("'", r"''\'''"),
        ("a\nb", "'a\nb'"),
        ("$HOME", "'$HOME'"),
        ("~", "'~'"),
        ("é", "'é'"),
    ];
    for (value, quoted) in cases {
        assert_eq!(shell_quote(value), quoted, "{value:?}");
    }
}

#[test]
fn unknown_placeholders_and_stray_braces_are_rejected() {
    for command in
        ["mytool {session}", "mytool {}", "mytool {session_id", "mytool session_id}", "mytool {{session_id}", "mytool {Session_Id}"]
    {
        let verdict = parse(&format!("resume: {{command: \"{command}\"}}"));
        match verdict {
            Err(Error::Spec(message)) => {
                assert!(message.contains("mytool.yaml") && message.contains("placeholder"), "{command}: {message}");
            }
            other => panic!("{command}: expected a spec error, got {other:?}"),
        }
    }
}

#[test]
fn an_empty_command_or_an_unknown_key_is_rejected() {
    assert!(matches!(parse("resume: {command: ' '}"), Err(Error::Spec(_))));
    assert!(matches!(parse("resume: {}"), Err(Error::Spec(_))));
    assert!(matches!(parse("resume: {command: x, shell: fish}"), Err(Error::Spec(_))));
}

#[test]
fn every_documented_placeholder_is_accepted() {
    for name in RESUME_PLACEHOLDERS {
        assert!(parse(&format!("resume: {{command: \"mytool {{{name}}}\"}}")).is_ok(), "{name}");
    }
}

/// The commands checked against each tool's own documentation (`verified` says when and where); a
/// tool left out has no template on purpose (see its spec).
#[test]
fn bundled_templates_are_the_verified_ones() {
    let expected = [
        ("claude-code", Some("claude --resume abc")),
        ("codex", Some("codex resume abc")),
        ("copilot-cli", Some("copilot --resume=abc")),
        ("cursor", None),
        ("claude-code-otel", None),
    ];
    for (id, command) in expected {
        let spec = bundled_spec(id);
        assert_eq!(spec.resume_command("abc", Some("/work")).as_deref(), command, "{id}");
        if let Some(resume) = &spec.resume {
            assert!(!resume.verified.trim().is_empty(), "{id} must say what its command was checked against");
        }
    }
}

#[test]
fn the_line_to_paste_goes_to_the_folder_first_with_it_quoted() {
    let spec = with_command("mytool --resume {session_id}");

    let plain = spec.resume_action("abc", Some("/work/app")).unwrap();
    assert_eq!(
        (plain.command.as_str(), plain.cwd.as_deref(), plain.paste.as_str()),
        ("mytool --resume abc", Some("/work/app"), "cd /work/app && mytool --resume abc")
    );

    let awkward = spec.resume_action("abc", Some("/my work/o'neil")).unwrap();
    assert_eq!(awkward.cwd.as_deref(), Some("/my work/o'neil"), "the folder is kept as it is");
    assert_eq!(awkward.paste, r"cd '/my work/o'\''neil' && mytool --resume abc");
}

#[test]
fn without_a_folder_the_line_to_paste_is_the_command() {
    let spec = with_command("mytool --resume {session_id}");
    for cwd in [None, Some("")] {
        let resume = spec.resume_action("abc", cwd).unwrap();
        assert_eq!((resume.cwd, resume.paste.as_str()), (None, "mytool --resume abc"), "{cwd:?}");
    }
}

#[test]
fn no_template_gives_no_action() {
    assert_eq!(bundled_spec("cursor").resume_action("abc", Some("/work")), None);
}

#[test]
fn line_is_the_paste_the_cli_prints_and_the_dashboard_copies() {
    let spec = with_command("mytool --resume {session_id}");
    let with = spec.resume_action("abc", Some("/work/my app")).unwrap();
    assert_eq!(with.line(), "cd '/work/my app' && mytool --resume abc");
    assert_eq!(with.line(), with.paste);
    assert_eq!(spec.resume_action("abc", None).unwrap().line(), "mytool --resume abc");
}
