//! JSON-config mechanics: render, merge, strip, uninstall, read, dump, diff, write with backup.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use jiff::civil::date;
use common::*;
use gitprompt_capture::installer::{self, Record};
use gitprompt_core::Error;
use gitprompt_core::spec::Pattern;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn ours() -> Value {
    json!({"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:7878/ingest/claude-code"}]}]}})
}

fn user() -> Value {
    json!({"theme": "dark", "hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": "./mine.sh"}]}], "Stop": []}})
}

fn marker() -> Pattern {
    TestEnv::new().spec("claude-code").marker()
}

// --- render ----------------------------------------------------------------------------------

#[test]
fn render_fills_placeholders_in_every_string_and_leaves_keys_and_other_values() {
    let template = json!({"{{source}}": ["{{receiver}}/ingest/{{source}}", 5, true, {"h": "Bearer {{token}} {{unknown}}"}]});
    let values = [("receiver", "http://127.0.0.1:7878"), ("source", "claude-code"), ("token", "t0k")];
    assert_eq!(
        installer::render(&template, &values),
        json!({"{{source}}": ["http://127.0.0.1:7878/ingest/claude-code", 5, true, {"h": "Bearer t0k {{unknown}}"}]})
    );
}

// --- merge and strip -------------------------------------------------------------------------

#[test]
fn merge_appends_our_entry_and_strip_restores_the_original() {
    let merged = installer::merge(&user(), &ours());
    assert_eq!(merged["hooks"]["UserPromptSubmit"].as_array().unwrap().len(), 2);
    assert_eq!(installer::strip(&merged, &marker()), user());
}

#[test]
fn strip_removes_only_the_containers_it_emptied() {
    let merged = installer::merge(&json!({"x": 1}), &ours());
    assert_eq!(installer::strip(&merged, &marker()), json!({"x": 1}));
}

#[test]
fn strip_removes_an_object_member_whose_value_is_our_url() {
    let doc = json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:7878/ingest/claude-code", "KEEP": "1"}});
    assert_eq!(installer::strip(&doc, &marker()), json!({"env": {"KEEP": "1"}}));
}

#[test]
fn strip_leaves_entries_of_a_source_whose_id_merely_starts_the_same() {
    let doc = json!({"hooks": {"Stop": [{"hooks": [{"command": "curl http://127.0.0.1:7878/ingest/claude-code-otel"}]}]}});
    assert_eq!(installer::strip(&doc, &marker()), doc);
}

#[test]
fn merge_is_idempotent() {
    let once = installer::merge(&user(), &ours());
    assert_eq!(installer::merge(&once, &ours()), once);
}

#[test]
fn merge_keeps_an_existing_plain_value_the_user_set() {
    let merged = installer::merge(&json!({"env": {"A": "mine"}}), &json!({"env": {"A": "ours", "B": "ours"}}));
    assert_eq!(merged, json!({"env": {"A": "mine", "B": "ours"}}));
}

#[test]
fn the_marker_does_not_match_a_longer_source_id() {
    let marker = marker();
    assert!(marker.0.is_match("http://x/ingest/claude-code").unwrap());
    assert!(!marker.0.is_match("http://x/ingest/claude-code-otel").unwrap());
}

// --- added and uninstall ---------------------------------------------------------------------

#[test]
fn uninstall_removes_only_values_we_added_and_the_user_left_unchanged() {
    let user = json!({"env": {"KEEP": "1", "CLAUDE_CODE_ENABLE_TELEMETRY": "1"}});
    let patch = json!({"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1", "OTEL_LOGS_EXPORTER": "otlp", "OTEL_PROTOCOL": "http/json"}});

    let record = installer::added(&user, &patch);
    assert_eq!(
        record,
        vec![
            (vec!["env".to_string(), "OTEL_LOGS_EXPORTER".to_string()], json!("otlp")),
            (vec!["env".to_string(), "OTEL_PROTOCOL".to_string()], json!("http/json"))
        ]
    );

    let mut merged = obj(installer::merge(&user, &patch));
    merged["env"]["OTEL_PROTOCOL"] = json!("grpc"); // edited by the user after connecting: kept
    assert_eq!(
        Value::Object(installer::uninstall(&merged, &marker(), &record)),
        json!({"env": {"KEEP": "1", "CLAUDE_CODE_ENABLE_TELEMETRY": "1", "OTEL_PROTOCOL": "grpc"}})
    );
}

#[test]
fn uninstall_removes_objects_that_only_held_values_we_added() {
    let patch = json!({"env": {"OTEL_LOGS_EXPORTER": "otlp"}});
    let merged = obj(installer::merge(&json!({}), &patch));
    let record = installer::added(&json!({}), &patch);
    assert_eq!(Value::Object(installer::uninstall(&merged, &marker(), &record)), json!({}));
}

#[test]
fn a_record_reads_and_writes_in_the_python_daemons_format() {
    let record: Record = serde_json::from_str(r#"[[["env", "OTEL_LOGS_EXPORTER"], "otlp"]]"#).unwrap();
    assert_eq!(record, vec![(vec!["env".to_string(), "OTEL_LOGS_EXPORTER".to_string()], json!("otlp"))]);
    assert_eq!(serde_json::to_value(&record).unwrap(), json!([[["env", "OTEL_LOGS_EXPORTER"], "otlp"]]));
}

// --- read, dump, diff, write -----------------------------------------------------------------

#[test]
fn read_refuses_invalid_json_and_names_the_file() {
    let env = TestEnv::new();
    let path = env.path("settings.json");
    write_text(&path, "{ // comment\n}");
    let err = installer::read(&path).unwrap_err();
    assert!(matches!(&err, Error::Config(m) if m.contains("settings.json") && m.contains("not valid JSON")), "{err:?}");
}

#[test]
fn read_refuses_a_file_that_does_not_hold_an_object() {
    let env = TestEnv::new();
    let path = env.path("settings.json");
    write_text(&path, "[1, 2]");
    assert!(matches!(installer::read(&path), Err(Error::Config(m)) if m.contains("does not hold a JSON object")));
}

#[test]
fn read_treats_a_missing_file_as_empty_and_a_blank_file_as_an_empty_object() {
    let env = TestEnv::new();
    let blank = env.path("blank.json");
    write_text(&blank, "  \n");
    let (text, doc) = installer::read(&env.path("missing.json")).unwrap();
    assert_eq!((text.as_str(), doc.len()), ("", 0));
    let (text, doc) = installer::read(&blank).unwrap();
    assert_eq!((text.as_str(), doc.len()), ("  \n", 0));
}

#[test]
fn dump_indents_by_two_keeps_non_ascii_and_ends_with_a_newline() {
    let doc = obj(json!({"name": "café", "list": [1]}));
    assert_eq!(installer::dump(&doc), "{\n  \"name\": \"café\",\n  \"list\": [\n    1\n  ]\n}\n");
}

#[test]
fn diff_is_empty_for_identical_texts() {
    assert_eq!(installer::diff(std::path::Path::new("/x/settings.json"), "{}\n", "{}\n"), "");
}

#[test]
fn diff_is_a_unified_diff_naming_the_file_on_both_sides() {
    let before = "{\n  \"theme\": \"dark\"\n}\n";
    let after = "{\n  \"theme\": \"dark\",\n  \"x\": 1\n}\n";
    let diff = installer::diff(std::path::Path::new("/x/settings.json"), before, after);
    assert_eq!(
        diff,
        "--- /x/settings.json\n+++ /x/settings.json\n@@ -1,3 +1,4 @@\n {\n-  \"theme\": \"dark\"\n+  \"theme\": \"dark\",\n+  \"x\": 1\n }\n"
    );
}

#[test]
fn write_backs_up_the_old_file_under_a_timestamped_name_and_keeps_its_mode() {
    let env = TestEnv::new();
    let path = env.path("settings.json");
    write_text(&path, "{\"old\": true}");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let now = date(2026, 11, 7).at(10, 11, 12, 0);
    let backups = env.path("gitprompt/backups");

    let backup = installer::write(&path, "{}\n", &backups, now).unwrap();

    assert_eq!(backup, Some(backups.join("settings.json.20261107-101112")));
    assert_eq!(fs::read_to_string(backups.join("settings.json.20261107-101112")).unwrap(), "{\"old\": true}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "{}\n");
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o640);
    assert_eq!(fs::metadata(&backups).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn write_creates_a_new_file_with_mode_0600_and_no_backup() {
    let env = TestEnv::new();
    let path = env.path("new/hooks.json");
    let now = date(2026, 11, 7).at(0, 0, 0, 0);

    let backup = installer::write(&path, "{}\n", &env.path("gitprompt/backups"), now).unwrap();

    assert_eq!(backup, None);
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(!env.path("gitprompt/backups").exists());
}
