//! Rotating the receiver token reconnects every connected source, after the same dry-run review as
//! Connect; it is refused when a connected source could not be reconnected.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use common::*;
use gitprompt_capture::connection::Connection;
use gitprompt_capture::rotation::{Rotation, rotate, rotate_via};
use gitprompt_core::Error;
use pretty_assertions::assert_eq;
use serde_json::json;

fn connected_claude(env: &TestEnv) -> std::path::PathBuf {
    let file = env.claude_settings();
    write_json(&file, &json!({}));
    Connection::new(&env.spec("claude-code"), &env.settings).connect(false).unwrap();
    file
}

#[test]
fn a_dry_run_previews_each_connected_source_with_a_placeholder_and_changes_nothing() {
    let env = TestEnv::new();
    let file = connected_claude(&env);
    let old = env.settings.token().unwrap();

    let preview = rotate(&env.specs(), &env.settings, true).unwrap();

    assert!(!preview.applied && preview.blocked.is_empty());
    let sources: Vec<(&str, &str)> = preview.reconnects.iter().map(|r| (r.source.as_str(), r.name.as_str())).collect();
    assert_eq!(sources, vec![("claude-code", "Claude Code")]);
    assert!(preview.reconnects[0].change.diff.contains("<new token>"), "{}", preview.reconnects[0].change.diff);
    assert!(!preview.reconnects[0].change.applied);
    assert_eq!(env.settings.token().unwrap(), old);
    assert!(fs::read_to_string(&file).unwrap().contains(&old));
}

#[test]
fn applying_rotates_the_token_and_rewrites_every_connected_entry() {
    let env = TestEnv::new();
    let file = connected_claude(&env);
    let old = env.settings.token().unwrap();

    let applied = rotate(&env.specs(), &env.settings, false).unwrap();

    let new = env.settings.token().unwrap();
    let written = fs::read_to_string(&file).unwrap();
    assert!(applied.applied && new != old);
    assert!(written.contains(&new) && !written.contains(&old));
    assert!(applied.reconnects[0].change.backup.is_some());
    assert_eq!(fs::metadata(env.settings.token_file()).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn rotation_with_nothing_connected_only_changes_the_token() {
    let env = TestEnv::new();
    let old = env.settings.token().unwrap();
    let applied = rotate(&env.specs(), &env.settings, false).unwrap();
    assert_eq!(applied, Rotation { reconnects: vec![], blocked: vec![], applied: true });
    assert_ne!(env.settings.token().unwrap(), old);
}

#[test]
fn a_connected_source_blocked_by_policy_is_listed_in_the_preview_and_refuses_the_rotation() {
    let env = TestEnv::new();
    let file = connected_claude(&env);
    let mut doc = read_json(&file);
    doc["disableAllHooks"] = json!(true);
    write_json(&file, &doc);
    let old = env.settings.token().unwrap();

    let preview = rotate(&env.specs(), &env.settings, true).unwrap();
    let refused = rotate(&env.specs(), &env.settings, false).unwrap_err();

    assert_eq!(preview.reconnects, vec![]);
    assert_eq!(preview.blocked, vec![format!("Claude Code: disableAllHooks is set in {}", file.display())]);
    assert!(matches!(&refused, Error::PolicyBlocked(m) if m.contains("Disconnect it first, then rotate")), "{refused:?}");
    assert_eq!(env.settings.token().unwrap(), old);
}

#[test]
fn an_unreadable_connected_config_fails_the_rotation_before_the_token_changes() {
    let env = TestEnv::new();
    let file = connected_claude(&env);
    let text = fs::read_to_string(&file).unwrap();
    write_text(&file, &format!("{text}\n// broken"));
    let old = env.settings.token().unwrap();

    let err = rotate(&env.specs(), &env.settings, false).unwrap_err();

    assert!(matches!(err, Error::Config(_)), "{err:?}");
    assert_eq!(env.settings.token().unwrap(), old);
}

#[test]
fn rotating_keeps_forwarder_hooks_in_their_form() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &json!({}));
    let forwarder = std::path::Path::new("/opt/gitprompt/bin/gitprompt");
    Connection::new(&env.spec("claude-code"), &env.settings).with_forwarder(forwarder).connect(false).unwrap();
    let before = fs::read_to_string(&file).unwrap();

    let preview = rotate_via(&env.specs(), &env.settings, true, Some(forwarder)).unwrap();
    let applied = rotate_via(&env.specs(), &env.settings, false, Some(forwarder)).unwrap();

    assert_eq!(preview.reconnects[0].change.diff, "", "forwarder hooks carry no token");
    assert!(applied.applied && !applied.reconnects[0].change.applied);
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
}
