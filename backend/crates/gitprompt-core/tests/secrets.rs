//! Secret detection and redaction (port of insights/redaction.py and its test in `test_insights.py`).
//! Sample secrets are assembled from parts so they never sit whole in the source.
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

use gitprompt_core::secrets::{KINDS, find_secrets, hide, redact};
use pretty_assertions::assert_eq;
use serde_json::json;

fn aws() -> String {
    "AKIA".to_owned() + "ABCDEFGHIJKLMNOP"
}

/// One sample per kind, in `KINDS` order, each detected as that kind alone.
fn samples() -> Vec<(&'static str, String)> {
    vec![
        ("private key", "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\nIBAAK\n-----END RSA PRIVATE KEY-----".to_owned()),
        ("AWS access key", aws()),
        ("GitHub token", "ghp_".to_owned() + &"a1B2".repeat(9)),
        ("Anthropic API key", "sk-ant-".to_owned() + "api03-abcdefghijklmnopqrst"),
        ("OpenAI API key", "sk-".to_owned() + "proj-abcdefghijklmnopqrstuv"),
        ("Slack token", "xoxb-".to_owned() + "1234567890-abcdef"),
        ("Google API key", "AIza".to_owned() + &"Sy0_-abcde".repeat(3) + "fghij"),
        ("Stripe key", "sk_live_".to_owned() + &"abcd1234".repeat(3)),
        ("JWT", "eyJ".to_owned() + "hbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abcdefghijk"),
        ("GitLab token", "glpat-".to_owned() + "x9Kq2Lm7Vt4Rw8Zp3Nc6"),
        ("npm token", "npm_".to_owned() + &"aB3dE6gH9k".repeat(3) + "mN2pQ5"),
        ("PyPI token", "pypi-AgEIcHlwaS5vcmc".to_owned() + &"Qx7Lm2Vt9Rw4Zp8Nc3Kb".repeat(3)),
        ("Hugging Face token", "hf_".to_owned() + &"QxLmVtRwZpNcKbYdFgHj".repeat(2)),
        ("SendGrid key", "SG.".to_owned() + &"a".repeat(22) + "." + &"b".repeat(43)),
        ("Atlassian token", "ATATT3".to_owned() + &"xFfGF0q8Zt2Lw9XkP4vR7mN3cB".repeat(3)),
        ("credentials in a URL", "postgres://app:".to_owned() + "Tr0ub4dor3@db.internal"),
        ("bearer token", "Authorization: Bearer ".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6"),
        ("password or key assignment", "password=".to_owned() + "hunter2hunter2"),
    ]
}

#[test]
fn samples_cover_every_kind_in_order() {
    let names: Vec<&str> = samples().iter().map(|(name, _)| *name).collect();
    let kinds: Vec<&str> = KINDS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, kinds);
}

#[test]
fn each_kind_of_secret_is_found_by_its_name_alone() {
    let found: Vec<(&str, Vec<&str>)> = samples().iter().map(|(name, text)| (*name, find_secrets(&format!("use {text} please")))).collect();
    let expected: Vec<(&str, Vec<&str>)> = samples().iter().map(|(name, _)| (*name, vec![*name])).collect();
    assert_eq!(found, expected);
}

#[test]
fn each_kind_of_secret_is_hidden_keeping_the_text_around_it() {
    let hidden: Vec<String> = samples().iter().map(|(_, text)| hide(&format!("use {text} please"))).collect();
    let expected: Vec<String> = samples().iter().map(|(name, _)| format!("use [hidden {name}] please")).collect();
    assert_eq!(hidden, expected);
}

#[test]
fn secrets_are_named_and_redacted_without_changing_structure() {
    let key = aws();
    assert_eq!(find_secrets(&format!("use {key} please")), ["AWS access key"]);
    assert_eq!(redact(&json!({"a": [format!("k={key}")], "n": 1})), json!({"a": ["k=[hidden AWS access key]"], "n": 1}));
}

#[test]
fn text_without_secrets_finds_nothing_and_is_unchanged() {
    let text = "Refactor the token parser; the password field is optional. sk-short AKIA123";
    assert_eq!(find_secrets(text), Vec::<&str>::new());
    assert_eq!(hide(text), text);
}

#[test]
fn several_kinds_are_listed_in_detection_order_not_text_order() {
    let text = format!("{} then {}", "xoxb-".to_owned() + "1234567890-abcdef", aws());
    assert_eq!(find_secrets(&text), ["AWS access key", "Slack token"]);
}

#[test]
fn a_kind_found_twice_is_named_once_and_hidden_twice() {
    let text = format!("{} and {}", aws(), "ASIA".to_owned() + "ZYXWVUTSRQPONMLK");
    assert_eq!(find_secrets(&text), ["AWS access key"]);
    assert_eq!(hide(&text), "[hidden AWS access key] and [hidden AWS access key]");
}

#[test]
fn a_private_key_block_is_hidden_whole() {
    let (_, block) = &samples()[0];
    assert_eq!(hide(&format!("key:\n{block}\nafter")), "key:\n[hidden private key]\nafter");
}

#[test]
fn an_unterminated_private_key_is_hidden_to_the_end_of_the_text() {
    let text = "key: -----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk\nmore lines";
    assert_eq!(find_secrets(text), ["private key"]);
    assert_eq!(hide(text), "key: [hidden private key]");
}

#[test]
fn an_anthropic_key_is_not_also_reported_as_an_openai_key() {
    // OpenAI's pattern excludes `sk-ant-` with a negative look-ahead.
    let key = "sk-ant-".to_owned() + "api03-abcdefghijklmnopqrst";
    assert_eq!(find_secrets(&key), ["Anthropic API key"]);
}

#[test]
fn an_openai_key_without_project_prefix_is_found() {
    assert_eq!(find_secrets(&("sk-".to_owned() + "abcdefghijklmnopqrstuvwx")), ["OpenAI API key"]);
}

#[test]
fn an_aws_key_needs_exactly_sixteen_characters_at_word_boundaries() {
    assert_eq!(find_secrets(&("AKIA".to_owned() + "ABCDEFGHIJKLMNO")), Vec::<&str>::new()); // 15
    assert_eq!(find_secrets(&("AKIA".to_owned() + "ABCDEFGHIJKLMNOPQ")), Vec::<&str>::new()); // 17: no boundary
    assert_eq!(find_secrets(&("X".to_owned() + &aws())), Vec::<&str>::new()); // no boundary before
}

#[test]
fn a_github_token_needs_at_least_thirty_six_characters() {
    assert_eq!(find_secrets(&("ghp_".to_owned() + &"a".repeat(35))), Vec::<&str>::new());
    assert_eq!(find_secrets(&("ghs_".to_owned() + &"a".repeat(36))), ["GitHub token"]);
    assert_eq!(find_secrets(&("github_pat_".to_owned() + &"a_1".repeat(8))), ["GitHub token"]);
}

#[test]
fn a_password_assignment_ignores_case_and_quotes_but_needs_eight_characters() {
    assert_eq!(find_secrets(&("API_KEY: '".to_owned() + "abcdefgh'")), ["password or key assignment"]);
    assert_eq!(find_secrets(&("Api-Key = \"".to_owned() + "abcdefgh\"")), ["password or key assignment"]);
    assert_eq!(find_secrets(&("PWD=".to_owned() + "abcdefgh")), ["password or key assignment"]);
    assert_eq!(find_secrets(&("password=".to_owned() + "abcdefg")), Vec::<&str>::new());
    assert_eq!(hide(&("token: \"".to_owned() + "abcdefgh\" end")), "[hidden password or key assignment]\" end");
}

#[test]
fn redact_hides_secrets_in_nested_strings_and_leaves_keys_alone() {
    let key = aws();
    let value = json!({key.clone(): {"list": [key, 3, null, false]}});
    let expected = json!({key: {"list": ["[hidden AWS access key]", 3, null, false]}});
    assert_eq!(redact(&value), expected);
}

use gitprompt_core::leak::masked;
use gitprompt_core::secrets::find_leaks;

/// The kinds and masked values of the plausible secrets in `text`.
fn leaks(text: &str) -> Vec<(&'static str, String)> {
    find_leaks(text).into_iter().map(|f| (f.kind, masked(f.mask, f.value))).collect()
}

#[test]
fn real_looking_secrets_are_leaks_with_only_their_masked_value() {
    let key = "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd";
    assert_eq!(leaks(&format!("export OPENAI_API_KEY={key}")), [("OpenAI API key", "sk-…y6Hd".to_owned())]);
    assert_eq!(leaks("db_password = 'Tr0ub4dor&3x'"), [("password or key assignment", "T…".to_owned())]);
    assert_eq!(leaks("postgres://app:Tr0ub4dor3xZ@db.internal/app"), [("credentials in a URL", "T…".to_owned())]);
    let block = format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{}\n-----END OPENSSH PRIVATE KEY-----", "b3BlbnNzaC1rZXktdjEAAAAA".repeat(4));
    assert_eq!(leaks(&block), [("private key", "hidden".to_owned())]);
}

#[test]
fn placeholders_examples_and_code_that_names_a_secret_are_not_leaks() {
    let not_leaks = [
        "AKIAIOSFODNN7EXAMPLE",
        "OPENAI_API_KEY=sk-xxxxxxxxxxxxxxxxxxxxxxxx",
        "api_key = os.environ[\"OPENAI_API_KEY\"]",
        "password: ${DB_PASSWORD}",
        "token = config.auth.token_value",
        "password=YOUR_PASSWORD_HERE",
        "secret: <your-secret>",
        "password = PASSWORD_FROM_ENV",
        "https://user:${TOKEN}@github.com/acme/app.git",
        "-----BEGIN RSA PRIVATE KEY----- goes here, then -----END RSA PRIVATE KEY-----",
        "| { type: \"link\"; token: LinkToken }",
        "logs on a wrong password: \"observatory: failed login\"",
        "SESSION_SECRET: CFG.sessionSecret,",
        "#   token: https://id.atlassian.com/manage/api-tokens",
        "ACCOUNT_PASSWORD = me@corp.io",
        "\"prompt\": \"key password=hunter2hunter2\"",
        "new URL('https://x.io/?token=supersecret'));",
        "blobToken: target.config.blobToken,",
        "the read-write token: BLOB_READ_WRITE_TOKEN. Without it",
        "PWD=/Users/me/Desktop/git/app OLDPWD=/Users/me",
        "POSTGRES_PASSWORD=\nPOSTGRES_URL=postgres://h/db",
        "GLIMMER_API_KEY= GLIMMER_MODEL=qwen-30B",
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    ];
    for text in not_leaks {
        assert_eq!(leaks(text), [], "{text}");
    }
}
