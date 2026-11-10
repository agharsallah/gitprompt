//! The one redaction pass (export/redact.py, ADR-0018 §2): which keys go, what is masked inside
//! strings, and how paths become relative. Works on JSON, so it is tested on JSON directly.

mod common;

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::*;
use gitprompt_export::redact::{Options, redact, roots_in};

fn none() -> BTreeSet<String> {
    BTreeSet::new()
}

fn roots(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn default_redact(value: Value) -> Value {
    redact(&value, &Options::default(), None, &none())
}

// --- options -------------------------------------------------------------------------------------

#[test]
fn options_default_to_the_most_private_choice() {
    assert_eq!(Options::default(), Options { prompt_text: false, relative_paths: true, drop_identity: true, drop_raw: true });
}

#[test]
fn options_are_stated_as_json_in_declaration_order() {
    let stated = serde_json::to_string(&Options::default().as_json()).unwrap();
    assert_eq!(stated, r#"{"prompt_text":false,"relative_paths":true,"drop_identity":true,"drop_raw":true}"#);
}

#[test]
fn options_missing_from_a_request_take_their_private_default() {
    let parsed: Options = serde_json::from_value(json!({"prompt_text": true})).unwrap();
    assert_eq!(parsed, Options { prompt_text: true, ..Options::default() });
}

// --- the Python rules, ported as is --------------------------------------------------------------

#[test]
fn defaults_drop_text_raw_and_identity_and_make_repo_paths_relative() {
    let value = json!({
        "repo": REPO, "files": [format!("{REPO}/a.py"), REPO, format!("{REPO}x/b.py")],
        "raw": {"k": 1}, "text": "t", "user.id": "u1",
    });
    assert_eq!(default_redact(value), json!({"repo": "shop", "files": ["a.py", "shop", "~/code/shopx/b.py"]}));
}

#[test]
fn every_option_opened_gives_the_value_back_unchanged() {
    let value = json!({
        "repo": REPO, "files": [format!("{REPO}/a.py"), REPO, format!("{REPO}x/b.py")],
        "raw": {"k": 1}, "text": "t", "user.id": "u1",
    });
    assert_eq!(redact(&value, &all_open(), None, &none()), value);
}

#[test]
fn secrets_are_hidden_with_every_option_opened() {
    let value = json!({"note": format!("key={SECRET}")});
    assert_eq!(redact(&value, &all_open(), None, &none()), json!({"note": "key=[hidden Anthropic API key]"}));
}

#[test]
fn any_users_home_folder_becomes_a_tilde() {
    assert_eq!(default_redact(json!("other home /home/alex/x and /Users/bo")), json!("other home ~/x and ~"));
}

#[test]
fn a_home_folder_written_as_one_path_component_is_dropped() {
    assert_eq!(default_redact(json!("~/.claude/projects/-Users-bo-code-app/m.md")), json!("~/.claude/projects/…/m.md"));
}

#[test]
fn a_clipped_text_loses_its_cut_last_token() {
    assert_eq!(default_redact(json!("starts the key sk-ant-api03-abcd…")), json!("starts the key …"));
}

// --- secrets -------------------------------------------------------------------------------------

#[test]
fn secrets_are_hidden_in_nested_arrays_and_object_keys() {
    let value = json!({"a": [[format!("token {AWS_KEY}")]], SECRET: 1});
    assert_eq!(
        redact(&value, &all_open(), None, &none()),
        json!({"a": [["token [hidden AWS access key]"]], "[hidden Anthropic API key]": 1})
    );
}

#[test]
fn a_short_cut_token_is_kept() {
    assert_eq!(default_redact(json!("ok abc…")), json!("ok abc…"));
}

#[test]
fn a_cut_token_is_only_removed_at_the_end_of_the_text() {
    assert_eq!(default_redact(json!("abcdefgh… more")), json!("abcdefgh… more"));
}

// --- text, raw and identity keys -----------------------------------------------------------------

#[test]
fn text_keys_are_dropped_by_default() {
    let value = json!({
        "text": 1, "typed_text": 1, "prompt": 1, "excerpt": 1, "label": 1, "title": 1, "snippet": 1, "diff": 1, "q": 1,
        "kept": 1,
    });
    assert_eq!(default_redact(value), json!({"kept": 1}));
}

#[test]
fn text_keys_are_kept_with_prompt_text() {
    let value = json!({"text": "a", "typed_text": "b", "title": "c", "q": "d"});
    assert_eq!(redact(&value, &text_on(), None, &none()), value);
}

#[test]
fn raw_payload_keys_are_dropped_by_default() {
    let value = json!({"raw": {}, "fields": {}, "context": {}, "events": [], "kept": 1});
    assert_eq!(default_redact(value), json!({"kept": 1}));
}

#[test]
fn raw_payload_keys_are_kept_when_drop_raw_is_off() {
    let value = json!({"raw": {"a": 1}, "fields": {"b": 2}, "context": {}, "events": [3]});
    let options = Options { drop_raw: false, ..Options::default() };
    assert_eq!(redact(&value, &options, None, &none()), value);
}

#[test]
fn text_keys_inside_a_kept_raw_payload_still_follow_prompt_text() {
    let value = json!({"raw": {"prompt": "hi", "cwd": "/srv/x"}});
    let options = Options { drop_raw: false, ..Options::default() };
    assert_eq!(redact(&value, &options, None, &none()), json!({"raw": {"cwd": "/srv/x"}}));
}

#[test]
fn identity_keys_and_account_attributes_are_dropped_by_default() {
    let value = json!({
        "author": "a", "email": "b", "user_email": "c", "user_id": "d", "account_id": "e", "account_uuid": "f",
        "organization_id": "g", "user.email": "h", "organization.id": "i", "user.anything": "j", "kept": 1,
    });
    assert_eq!(default_redact(value), json!({"kept": 1}));
}

#[test]
fn identity_is_kept_when_drop_identity_is_off() {
    let value = json!({"author": AUTHOR, "user.email": ACCOUNT_EMAIL, "note": format!("mail {EMAIL}")});
    let options = Options { drop_identity: false, ..Options::default() };
    assert_eq!(redact(&value, &options, None, &none()), value);
}

#[test]
fn email_addresses_are_masked_in_every_string_by_default() {
    assert_eq!(
        default_redact(json!({"note": "mail sam@example.com or a.b+c@mail.example.co.uk"})),
        json!({"note": "mail [hidden email] or [hidden email]"})
    );
}

#[test]
fn email_addresses_are_masked_in_object_keys() {
    assert_eq!(default_redact(json!({EMAIL: 1})), json!({"[hidden email]": 1}));
}

#[test]
fn numbers_booleans_and_nulls_pass_unchanged() {
    let value = json!({"n": 3, "f": 1.5, "b": true, "z": null, "a": [false, 0]});
    assert_eq!(default_redact(value.clone()), value);
}

// --- paths ---------------------------------------------------------------------------------------

#[test]
fn the_given_home_folder_becomes_a_tilde() {
    let redacted = redact(&json!("/opt/people/sam/notes and /opt/people/sam"), &Options::default(), Some("/opt/people/sam"), &none());
    assert_eq!(redacted, json!("~/notes and ~"));
}

#[test]
fn a_home_folder_given_with_a_trailing_slash_is_matched_too() {
    let redacted = redact(&json!("/opt/people/sam/notes"), &Options::default(), Some("/opt/people/sam/"), &none());
    assert_eq!(redacted, json!("~/notes"));
}

#[test]
fn a_longer_folder_name_is_not_taken_for_the_home_folder() {
    let redacted = redact(&json!("/opt/people/samuel/x"), &Options::default(), Some("/opt/people/sam"), &none());
    assert_eq!(redacted, json!("/opt/people/samuel/x"));
}

#[test]
fn a_home_folder_of_slash_is_ignored() {
    let redacted = redact(&json!("/etc/hosts"), &Options::default(), Some("/"), &none());
    assert_eq!(redacted, json!("/etc/hosts"));
}

#[test]
fn given_project_folders_make_paths_relative() {
    let redacted = redact(&json!(["/srv/work/api/main.go", "/srv/work/api"]), &Options::default(), None, &roots(&["/srv/work/api"]));
    assert_eq!(redacted, json!(["main.go", "api"]));
}

#[test]
fn the_longest_project_folder_wins() {
    let value = json!({"repo": "/srv/mono", "git_root": "/srv/mono/pkg", "file": "/srv/mono/pkg/x.ts", "other": "/srv/mono/y.ts"});
    assert_eq!(default_redact(value), json!({"repo": "mono", "git_root": "pkg", "file": "x.ts", "other": "y.ts"}));
}

#[test]
fn a_project_folder_followed_by_a_slash_and_a_space_keeps_its_name() {
    let redacted = redact(&json!("cd /srv/app/ now"), &Options::default(), None, &roots(&["/srv/app"]));
    assert_eq!(redacted, json!("cd app/ now"));
}

#[test]
fn a_root_of_slash_and_relative_repo_values_are_not_project_folders() {
    let value = json!({"repo": "/", "git_root": "shop", "f": "/etc/x", "g": "shop/a.py"});
    assert_eq!(default_redact(value.clone()), value);
}

#[test]
fn paths_stay_absolute_when_relative_paths_is_off() {
    let value = json!({"repo": REPO, "f": format!("{REPO}/a.py"), "g": "/Users/bo/x", "h": "-Users-bo-code"});
    let options = Options { relative_paths: false, ..Options::default() };
    assert_eq!(redact(&value, &options, Some(HOME), &none()), value);
}

#[test]
fn a_home_like_segment_inside_a_path_is_left_alone() {
    assert_eq!(default_redact(json!("src/home/page.tsx and backup-Users-bo")), json!("src/home/page.tsx and backup-Users-bo"));
}

#[test]
fn paths_inside_object_keys_are_made_relative() {
    assert_eq!(default_redact(json!({"/Users/bo/x": 1})), json!({"~/x": 1}));
}

// --- roots_in ------------------------------------------------------------------------------------

#[test]
fn roots_are_absolute_strings_under_root_keys_anywhere_without_trailing_slash() {
    let value = json!({
        "a": [{"repo": "/x/y/"}], "git_root": "/z", "git_worktree_root": 3, "nested": {"repo": "relative"}, "path": "/not/a/root",
    });
    assert_eq!(roots_in(&value), roots(&["/x/y", "/z"]));
}

#[test]
fn a_value_without_root_keys_has_no_roots() {
    assert_eq!(roots_in(&json!(["/a", {"b": "/c"}, 1])), none());
}

// --- tool metadata (a fix over the Python daemon) ------------------------------------------------

#[test]
fn a_tools_label_is_kept_by_default_inside_its_connector_and_surface() {
    // `label` is a text key, but in `connector`/`surface` it is the tool's short name: Python dropped
    // it, so the Tool column of every default export read "—".
    let value = json!({"connector": {"id": "codex", "label": "Codex"}, "surface": {"id": "cli", "label": "CLI"}});
    assert_eq!(default_redact(value.clone()), value);
}

#[test]
fn text_keys_outside_tool_metadata_are_still_dropped() {
    let value = json!({"label": "a subagent brief", "items": [{"label": "brief"}], "connector": {"title": "t", "label": "Codex"}});
    assert_eq!(default_redact(value), json!({"items": [{}], "connector": {"label": "Codex"}}));
}

#[test]
fn a_tools_label_still_has_its_secrets_emails_and_paths_masked() {
    let value = json!({"connector": {"label": format!("{SECRET} {EMAIL} {OTHER_HOME}/x")}});
    assert_eq!(default_redact(value), json!({"connector": {"label": "[hidden Anthropic API key] [hidden email] ~/x"}}));
}
