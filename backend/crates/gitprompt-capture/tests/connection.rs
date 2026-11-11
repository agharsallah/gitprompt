//! Connect and disconnect with consent (dry-run diff first), for every bundled tool, plus turning a
//! connector off and re-mapping its stored payloads. Real config files in a temporary user home.
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

use std::fs;
use std::os::unix::fs::PermissionsExt;

use common::*;
use gitprompt_capture::connection::{Connection, refuse_if_disabled, remap, set_enabled};
use gitprompt_capture::rotation::PLACEHOLDER;
use gitprompt_core::preferences::{PreferenceFile, Preferences};
use gitprompt_core::{Error, Json};
use pretty_assertions::assert_eq;
use serde_json::json;

fn handlers(doc: &Json, event: &str) -> Vec<Json> {
    doc["hooks"][event].as_array().unwrap_or_else(|| panic!("no {event} hooks in {doc}")).clone()
}

fn user_claude_settings() -> Json {
    json!({"theme": "dark", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}})
}

// --- Claude Code hooks -----------------------------------------------------------------------

#[test]
fn a_dry_run_shows_the_diff_and_writes_nothing() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &user_claude_settings());
    let before = fs::read_to_string(&file).unwrap();
    let spec = env.spec("claude-code");

    let preview = Connection::new(&spec, &env.settings).connect(true).unwrap();

    assert!(preview.diff.contains("/ingest/claude-code"), "diff: {}", preview.diff);
    assert_eq!((preview.applied, preview.backup, preview.file), (false, None, file.display().to_string()));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    assert!(!Connection::new(&spec, &env.settings).connected());
}

#[test]
fn connecting_adds_our_hooks_next_to_the_users_settings_and_backs_up_the_file() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &user_claude_settings());
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fs::read_to_string(&file).unwrap();
    let spec = env.spec("claude-code");

    let applied = Connection::new(&spec, &env.settings).connect(false).unwrap();

    let written = read_json(&file);
    assert!(applied.applied);
    assert_eq!(fs::read_to_string(applied.backup.expect("a backup")).unwrap(), before);
    assert_eq!(written["theme"], json!("dark"));
    assert_eq!(handlers(&written, "Stop").len(), 2, "the user's Stop hook and ours");
    assert_eq!(handlers(&written, "UserPromptSubmit").len(), 1);
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(Connection::new(&spec, &env.settings).connected());
    assert!(!Connection::new(&env.spec("claude-code-otel"), &env.settings).connected());
}

#[test]
fn disconnecting_restores_the_users_settings_exactly() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &user_claude_settings());
    let spec = env.spec("claude-code");
    Connection::new(&spec, &env.settings).connect(false).unwrap();

    let removed = Connection::new(&spec, &env.settings).disconnect(false).unwrap();

    assert!(removed.applied && removed.diff.contains('-'), "{removed:?}");
    assert_eq!(read_json(&file), user_claude_settings());
    assert!(!Connection::new(&spec, &env.settings).connected());
}

#[test]
fn connecting_again_changes_nothing() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &json!({}));
    let spec = env.spec("claude-code");
    Connection::new(&spec, &env.settings).connect(false).unwrap();
    let after_first = fs::read_to_string(&file).unwrap();

    let again = Connection::new(&spec, &env.settings).connect(false).unwrap();

    assert_eq!((again.diff.as_str(), again.applied), ("", false));
    assert_eq!(fs::read_to_string(&file).unwrap(), after_first);
}

#[test]
fn disconnecting_a_source_that_is_not_connected_changes_nothing() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let change = Connection::new(&spec, &env.settings).disconnect(false).unwrap();
    assert_eq!((change.diff.as_str(), change.applied), ("", false));
    assert!(!env.claude_settings().exists());
}

#[test]
fn connecting_creates_a_missing_config_file_without_a_backup() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let change = Connection::new(&spec, &env.settings).connect(false).unwrap();
    let file = env.claude_settings();
    assert_eq!((change.applied, change.backup), (true, None));
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn installed_hooks_are_async_commands_carrying_the_receiver_source_and_token() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let patch = Connection::new(&spec, &env.settings).patch().unwrap();
    let handler = &handlers(&patch, "PostToolUse")[0]["hooks"][0];
    let command = handler["command"].as_str().unwrap();

    assert_eq!((&handler["type"], &handler["async"]), (&json!("command"), &json!(true)));
    assert!(command.contains("http://127.0.0.1:7878/ingest/claude-code"), "{command}");
    assert!(command.contains(&format!("Bearer {}", env.settings.token().unwrap())), "{command}");
    assert!(!command.contains("{{"), "unrendered placeholder: {command}");
    assert!(spec.marker().0.is_match(command).unwrap(), "disconnect can find it");
}

#[test]
fn a_connection_can_be_rendered_with_another_token() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let patch = Connection::new(&spec, &env.settings).with_token(PLACEHOLDER).patch().unwrap();
    assert!(patch.to_string().contains("Bearer <new token>"));
}

#[test]
fn an_older_connection_is_reported_outdated_and_updated_in_place() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(
        &file,
        &json!({"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:7878/ingest/claude-code"}]}]}}),
    );
    let spec = env.spec("claude-code");
    let connection = Connection::new(&spec, &env.settings);
    assert!(connection.connected() && connection.outdated());

    connection.connect(false).unwrap();

    let written = read_json(&file);
    assert_eq!(handlers(&written, "UserPromptSubmit").len(), 1, "replaced, not duplicated");
    assert_eq!(handlers(&written, "Stop").len(), 1);
    assert!(!Connection::new(&spec, &env.settings).outdated());
}

#[test]
fn a_source_that_is_not_connected_or_unreadable_is_never_outdated() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    assert!(!Connection::new(&spec, &env.settings).outdated());
    write_text(&env.claude_settings(), "{oops http://127.0.0.1:7878/ingest/claude-code");
    assert!(!Connection::new(&spec, &env.settings).outdated());
}

#[test]
fn a_policy_block_refuses_to_connect_and_leaves_the_file_alone() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &json!({"disableAllHooks": true}));
    let before = fs::read_to_string(&file).unwrap();
    let spec = env.spec("claude-code");

    let dry = Connection::new(&spec, &env.settings).connect(true).unwrap_err();
    let real = Connection::new(&spec, &env.settings).connect(false).unwrap_err();

    assert!(matches!(&dry, Error::PolicyBlocked(m) if m.contains("disableAllHooks")), "{dry:?}");
    assert!(matches!(&real, Error::PolicyBlocked(m) if m.starts_with("Blocked by settings GitPrompt won't override: ")), "{real:?}");
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn invalid_settings_are_never_overwritten() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_text(&file, "{oops");
    let spec = env.spec("claude-code");
    let err = Connection::new(&spec, &env.settings).connect(false).unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err:?}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "{oops");
}

// --- Claude Code telemetry -------------------------------------------------------------------

#[test]
fn telemetry_connect_keeps_existing_env_and_disconnect_restores_only_what_it_added() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    let original = json!({"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1", "MY_VAR": "x"}});
    write_json(&file, &original);
    let (hooks, telemetry) = (env.spec("claude-code"), env.spec("claude-code-otel"));
    Connection::new(&hooks, &env.settings).connect(false).unwrap();
    Connection::new(&telemetry, &env.settings).connect(false).unwrap();

    let written = read_json(&file);
    assert_eq!(written["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"], json!("http://127.0.0.1:7878/ingest/claude-code-otel"));
    assert_eq!(written["env"]["MY_VAR"], json!("x"));
    assert!(Connection::new(&telemetry, &env.settings).connected());
    assert_eq!(Connection::new(&telemetry, &env.settings).connect(true).unwrap().diff, "");

    Connection::new(&telemetry, &env.settings).disconnect(false).unwrap();
    let mut expected = original.clone();
    expected["hooks"] = written["hooks"].clone();
    assert_eq!(read_json(&file), expected, "the hooks layer is untouched");

    Connection::new(&hooks, &env.settings).disconnect(false).unwrap();
    assert_eq!(read_json(&file), original);
}

#[test]
fn telemetry_is_not_redirected_when_already_exported_elsewhere() {
    let env = TestEnv::new();
    write_json(&env.claude_settings(), &json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://collector.example.com"}}));
    let spec = env.spec("claude-code-otel");
    let err = Connection::new(&spec, &env.settings).connect(false).unwrap_err();
    assert!(matches!(&err, Error::PolicyBlocked(m) if m.contains("OTEL_EXPORTER_OTLP_ENDPOINT")), "{err:?}");
}

#[test]
fn a_value_the_user_edited_after_connecting_survives_disconnect() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    let spec = env.spec("claude-code-otel");
    Connection::new(&spec, &env.settings).connect(false).unwrap();
    let mut doc = read_json(&file);
    doc["env"]["OTEL_LOGS_EXPORTER"] = json!("console");
    write_json(&file, &doc);

    Connection::new(&spec, &env.settings).disconnect(false).unwrap();

    assert_eq!(read_json(&file), json!({"env": {"OTEL_LOGS_EXPORTER": "console"}}));
}

#[test]
fn the_claude_config_directory_follows_claude_config_dir() {
    let env = TestEnv::with_env(&[]);
    let mut settings = env.settings.clone();
    let dir = env.path("claude-elsewhere");
    settings.env.set("CLAUDE_CONFIG_DIR", dir.to_string_lossy());
    let spec = env.spec("claude-code");
    assert_eq!(Connection::new(&spec, &settings).config_file(), dir.join("settings.json"));
    assert_eq!(Connection::new(&spec, &env.settings).config_file(), env.user_path(".claude/settings.json"));
}

// --- GitHub Copilot CLI ----------------------------------------------------------------------

#[test]
fn copilot_connect_writes_a_hooks_file_of_its_own_and_disconnect_empties_it() {
    let env = TestEnv::new();
    fs::create_dir_all(env.user_path(".copilot")).unwrap();
    let spec = env.spec("copilot-cli");
    let file = env.user_path(".copilot/hooks/gitprompt.json");

    assert!(Connection::new(&spec, &env.settings).connect(false).unwrap().applied);

    let written = read_json(&file);
    assert_eq!(written["version"], json!(1));
    let prompt_hooks = handlers(&written, "UserPromptSubmit");
    let bash = prompt_hooks[0]["bash"].as_str().unwrap();
    assert_eq!((prompt_hooks.len(), &prompt_hooks[0]["type"]), (1, &json!("command")));
    assert!(bash.contains("/ingest/copilot-cli") && !bash.contains("{{"), "{bash}");
    assert!(Connection::new(&spec, &env.settings).connected());
    assert!(!Connection::new(&env.spec("claude-code"), &env.settings).connected());

    Connection::new(&spec, &env.settings).disconnect(false).unwrap();
    assert_eq!(read_json(&file), json!({}));
    assert!(!Connection::new(&spec, &env.settings).connected());
}

#[test]
fn a_copilot_settings_file_that_turns_hooks_off_blocks_connecting() {
    let env = TestEnv::new();
    write_json(&env.user_path(".copilot/settings.json"), &json!({"disableAllHooks": true}));
    let spec = env.spec("copilot-cli");
    assert!(matches!(Connection::new(&spec, &env.settings).connect(false), Err(Error::PolicyBlocked(_))));
}

// --- Codex -----------------------------------------------------------------------------------

fn codex_settings(env: &TestEnv) -> (gitprompt_core::Settings, std::path::PathBuf) {
    let home = env.path("codex-home");
    fs::create_dir_all(&home).unwrap();
    let mut settings = env.settings.clone();
    settings.env.set("CODEX_HOME", home.to_string_lossy());
    (settings, home)
}

#[test]
fn codex_hooks_live_in_codex_home() {
    let env = TestEnv::new();
    let (settings, home) = codex_settings(&env);
    let spec = env.spec("codex");
    assert_eq!(Connection::new(&spec, &settings).config_file(), home.join("hooks.json"));
    assert_eq!(Connection::new(&spec, &env.settings).config_file(), env.user_path(".codex/hooks.json"));
}

#[test]
fn codex_connect_merges_into_the_users_hooks_file_and_disconnect_restores_it_byte_for_byte() {
    let env = TestEnv::new();
    let (settings, home) = codex_settings(&env);
    let hooks = home.join("hooks.json");
    let user =
        json!({"description": "mine", "hooks": {"PreToolUse": [{"matcher": "^Bash$", "hooks": [{"type": "command", "command": "true"}]}]}});
    let before = format!("{}\n", serde_json::to_string_pretty(&user).unwrap());
    write_text(&hooks, &before);
    let spec = env.spec("codex");

    assert!(Connection::new(&spec, &settings).connect(false).unwrap().applied);

    let written = read_json(&hooks);
    assert_eq!((&written["description"], &written["hooks"]["PreToolUse"]), (&json!("mine"), &user["hooks"]["PreToolUse"]));
    let prompt_hook = &handlers(&written, "UserPromptSubmit")[0]["hooks"][0];
    let end_hook = &handlers(&written, "SessionEnd")[0]["hooks"][0];
    let stop_hook = &handlers(&written, "Stop")[0]["hooks"][0];
    assert_eq!(prompt_hook["async"], json!(true));
    assert!(prompt_hook["command"].as_str().unwrap().contains("/ingest/codex"));
    assert!(end_hook.get("async").is_none() && end_hook["timeout"].as_u64().unwrap() <= 3, "SessionEnd runs synchronously, 3 s at most");
    assert!(stop_hook.get("async").is_none(), "an async Stop is cancelled when `codex exec` ends");
    assert!(Connection::new(&spec, &settings).connected());

    Connection::new(&spec, &settings).disconnect(false).unwrap();
    assert_eq!(fs::read_to_string(&hooks).unwrap(), before);
    assert!(!Connection::new(&spec, &settings).connected());
}

#[test]
fn codex_toml_that_turns_hooks_off_refuses_to_connect() {
    let env = TestEnv::new();
    let (settings, home) = codex_settings(&env);
    write_text(&home.join("config.toml"), "[features]\nhooks = false\n");
    let spec = env.spec("codex");
    assert!(matches!(Connection::new(&spec, &settings).connect(false), Err(Error::PolicyBlocked(_))));
    assert!(!home.join("hooks.json").exists());
}

// --- Cursor ----------------------------------------------------------------------------------

#[test]
fn cursor_handlers_are_plain_commands_with_a_timeout() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let patch = Connection::new(&spec, &env.settings).patch().unwrap();
    let subscribed = patch["hooks"].as_object().unwrap();
    let plain = |handlers: &Json| {
        let handlers = handlers.as_array().unwrap();
        handlers.len() == 1 && handlers[0].as_object().unwrap().keys().collect::<Vec<_>>() == ["command", "timeout"]
    };
    let not_plain: Vec<&String> = subscribed.iter().filter(|(_, h)| !plain(h)).map(|(event, _)| event).collect();
    assert!(!subscribed.is_empty());
    assert_eq!(not_plain, Vec::<&String>::new(), "one handler each, never failClosed, never a prompt-type hook");
}

#[test]
fn cursors_prompt_hook_lets_the_prompt_through_and_the_others_answer_empty() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let patch = Connection::new(&spec, &env.settings).patch().unwrap();
    let command = |event: &str| handlers(&patch, event)[0]["command"].as_str().unwrap().to_string();
    assert!(command("beforeSubmitPrompt").ends_with(r#"printf '%s' '{"continue":true}'"#), "{}", command("beforeSubmitPrompt"));
    assert!(command("stop").ends_with("printf '%s' '{}'"), "{}", command("stop"));
    assert!(command("stop").contains("/ingest/cursor"));
}

#[test]
fn cursor_subscribes_to_no_permission_hook() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let patch = Connection::new(&spec, &env.settings).patch().unwrap();
    let subscribed = patch["hooks"].as_object().unwrap();
    let permission = ["preToolUse", "beforeShellExecution", "beforeMCPExecution", "beforeReadFile", "subagentStart", "beforeTabFileRead"];
    let found: Vec<&&str> = permission.iter().filter(|e| subscribed.contains_key(**e)).collect();
    assert_eq!(found, Vec::<&&str>::new());
}

#[test]
fn cursor_connect_and_disconnect_round_trip_the_users_hooks_file() {
    let env = TestEnv::new();
    let file = env.user_path(".cursor/hooks.json");
    let user = json!({"version": 1, "hooks": {"afterFileEdit": [{"command": "./format.sh"}]}});
    write_json(&file, &user);
    let spec = env.spec("cursor");
    Connection::new(&spec, &env.settings).connect(false).unwrap();
    assert_eq!(read_json(&file)["hooks"]["afterFileEdit"], user["hooks"]["afterFileEdit"]);
    Connection::new(&spec, &env.settings).disconnect(false).unwrap();
    assert_eq!(read_json(&file), user);
}

// --- turned off, enable, disable -------------------------------------------------------------

#[test]
fn a_turned_off_connector_refuses_to_connect() {
    let env = TestEnv::new();
    let spec = env.spec("copilot-cli");
    let prefs = Preferences { disabled_sources: vec!["copilot-cli".into()], ..Preferences::default() };
    let err = refuse_if_disabled(&spec, &prefs).unwrap_err();
    assert!(matches!(&err, Error::Conflict(m) if m.contains("turned off in GitPrompt")), "{err:?}");
    assert!(refuse_if_disabled(&env.spec("claude-code"), &prefs).is_ok());
}

#[test]
fn disabling_and_enabling_a_connector_updates_only_its_entry_in_the_preferences() {
    let env = TestEnv::new();
    let prefs = PreferenceFile::new(&env.settings.home, env.settings.env.clone());
    let (copilot, otel) = (env.spec("copilot-cli"), env.spec("claude-code-otel"));

    assert_eq!(set_enabled(&copilot, &prefs, false).unwrap(), false);
    assert_eq!(set_enabled(&otel, &prefs, false).unwrap(), false);
    assert_eq!(prefs.current().unwrap().disabled_sources, vec!["copilot-cli", "claude-code-otel"]);

    assert_eq!(set_enabled(&copilot, &prefs, true).unwrap(), true);
    assert_eq!(prefs.current().unwrap().disabled_sources, vec!["claude-code-otel"]);
    set_enabled(&otel, &prefs, true).unwrap();
    assert_eq!(prefs.stored().unwrap().get("disabled_sources"), None, "an empty list removes the key");
}

#[test]
fn disabled_sources_set_by_the_environment_cannot_be_changed() {
    let env = TestEnv::with_env(&[("GITPROMPT_DISABLED_SOURCES", "copilot-cli, claude-code-otel")]);
    let prefs = PreferenceFile::new(&env.settings.home, env.settings.env.clone());
    let err = set_enabled(&env.spec("copilot-cli"), &prefs, true).unwrap_err();
    assert!(matches!(err, Error::Locked(_)), "{err:?}");
}

// --- remap -----------------------------------------------------------------------------------

#[test]
fn remap_applies_an_edited_spec_to_stored_payloads() {
    let env = TestEnv::new();
    let store = store();
    let mut spec = env.spec("claude-code");
    let payload = json!({"hook_event_name": "UserPromptSubmit", "session_id": "s", "prompt_id": "p1", "text": "typed"});
    let captured = ingest(&spec, &store, &payload);
    let gitprompt_capture::pipeline::StoredRow::Prompt(id) = only_row(&captured) else { panic!("not a prompt") };
    assert_eq!(prompt_row(&store, id).prompt.as_deref(), Some("typed"), "`text` is a default candidate");

    spec.map.insert("prompt".into(), vec!["nope".into()]);
    assert_eq!(remap(&spec, &store).unwrap(), 1);
    assert_eq!(prompt_row(&store, id).prompt, None);
}

#[test]
fn remapping_a_session_joined_source_keeps_its_prompt_ids_and_joins() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("copilot-cli");
    let send = |p: Json| ingest(&spec, &store, &p);
    let first = send(json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt": "Add a retry"}));
    send(json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_name": "bash"}));
    let second = send(json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "prompt": "/review"}));
    send(json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_name": "view"}));
    let ids = |c: &gitprompt_capture::Captured| match only_row(c) {
        gitprompt_capture::pipeline::StoredRow::Prompt(id) => id,
        other @ gitprompt_capture::pipeline::StoredRow::Event(_) => panic!("{other:?}"),
    };
    let (first, second) = (ids(&first), ids(&second));

    assert_eq!(remap(&spec, &store).unwrap(), 4, "two prompts and two events");

    assert_eq!(prompt_row(&store, first).fields["prompt_id"], json!(format!("copilot-cli#{first}")));
    let joined: Vec<(Option<String>, Option<String>)> = all_events(&store)
        .into_iter()
        .map(|e| (e.fields.get("tool_name").and_then(|t| t.as_str()).map(String::from), e.prompt_id))
        .collect();
    assert_eq!(
        joined,
        vec![(Some("bash".into()), Some(format!("copilot-cli#{first}"))), (Some("view".into()), Some(format!("copilot-cli#{second}")))]
    );
}

// --- delivering through GitPrompt's own forwarder (the desktop app) ---------------------------

const FORWARDER: &str = "/Applications/Git Prompt.app/Contents/MacOS/gitprompt";

#[test]
fn a_forwarder_connection_installs_hooks_that_run_the_forwarder_and_carry_the_marker() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let patch = Connection::new(&spec, &env.settings).with_forwarder(FORWARDER).patch().unwrap();
    let handler = &handlers(&patch, "Stop")[0]["hooks"][0];
    let command = handler["command"].as_str().unwrap();

    assert_eq!(command, "'/Applications/Git Prompt.app/Contents/MacOS/gitprompt' hook claude-code # /ingest/claude-code");
    assert_eq!((&handler["type"], &handler["async"]), (&json!("command"), &json!(true)), "the rest of the entry is the spec's");
    assert!(spec.marker().is_match(command), "connected/disconnect can find it");
    assert!(!patch.to_string().contains(&env.settings.token().unwrap()), "the forwarder reads the token itself");
}

#[test]
fn forwarder_hooks_connect_and_disconnect_like_curl_hooks() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &user_claude_settings());
    let spec = env.spec("claude-code");
    let connection = Connection::new(&spec, &env.settings).with_forwarder(FORWARDER);

    connection.connect(false).unwrap();

    let written = read_json(&file);
    assert_eq!(handlers(&written, "Stop").len(), 2, "the user's Stop hook and ours");
    assert!(connection.connected() && !connection.outdated());
    assert_eq!(connection.connect(true).unwrap().diff, "", "reconnecting changes nothing");

    connection.disconnect(false).unwrap();
    assert_eq!(read_json(&file), user_claude_settings());
}

#[test]
fn switching_between_curl_and_forwarder_hooks_is_an_update_in_place() {
    let env = TestEnv::new();
    write_json(&env.claude_settings(), &json!({}));
    let spec = env.spec("claude-code");
    Connection::new(&spec, &env.settings).connect(false).unwrap();
    let forwarder = Connection::new(&spec, &env.settings).with_forwarder(FORWARDER);
    assert!(forwarder.connected() && forwarder.outdated());

    forwarder.connect(false).unwrap();

    let written = read_json(&env.claude_settings());
    let commands: Vec<Json> = handlers(&written, "Stop").iter().map(|h| h["hooks"][0]["command"].clone()).collect();
    assert_eq!(commands, vec![json!("'/Applications/Git Prompt.app/Contents/MacOS/gitprompt' hook claude-code # /ingest/claude-code")]);
    assert!(!forwarder.outdated());
}

#[test]
fn cursors_forwarder_hooks_print_nothing_themselves_the_forwarder_replies() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let patch = Connection::new(&spec, &env.settings).with_forwarder(FORWARDER).patch().unwrap();
    let prompt = &handlers(&patch, "beforeSubmitPrompt")[0];
    assert_eq!(prompt["command"], json!("'/Applications/Git Prompt.app/Contents/MacOS/gitprompt' hook cursor # /ingest/cursor"));
    assert_eq!(prompt.as_object().unwrap().keys().collect::<Vec<_>>(), ["command", "timeout"]);
}

#[test]
fn values_that_are_not_hook_commands_are_rendered_as_usual_with_a_forwarder() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code-otel");
    let plain = Connection::new(&spec, &env.settings).patch().unwrap();
    let forwarded = Connection::new(&spec, &env.settings).with_forwarder(FORWARDER).patch().unwrap();
    assert_eq!(forwarded, plain);
}
