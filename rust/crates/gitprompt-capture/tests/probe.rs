//! What GitPrompt reads about a tool: detection, version (with an injected runner and clock),
//! capabilities missing from an older version, and policy from settings files and the environment.

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use gitprompt_capture::probe::{
    Clock, CommandRunner, PolicyReport, VersionCache, VersionProbe, check_policy, detected, missing_capabilities, version_of,
};
use gitprompt_core::spec::Capability;
use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::json;

// --- detection -------------------------------------------------------------------------------

#[test]
fn a_tool_is_detected_when_one_of_its_paths_exists() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    assert!(!detected(&spec, &env.settings));
    std::fs::create_dir_all(env.user_path(".claude")).unwrap();
    assert!(detected(&spec, &env.settings));
}

#[test]
fn detection_follows_the_tools_home_variable() {
    let env = TestEnv::new();
    let elsewhere = env.path("codex-home");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let mut settings = env.settings.clone();
    settings.env.set("CODEX_HOME", elsewhere.to_string_lossy());
    assert!(detected(&env.spec("codex"), &settings));
    assert!(!detected(&env.spec("codex"), &env.settings));
}

// --- version ---------------------------------------------------------------------------------

#[test]
fn the_version_is_the_first_dotted_number_of_the_output() {
    assert_eq!(version_of("2.1.317 (Claude Code)").as_deref(), Some("2.1.317"));
    assert_eq!(version_of("GitHub Copilot CLI 1.0.102.\n").as_deref(), Some("1.0.102"));
    assert_eq!(version_of("codex-cli 0.167.0-alpha.2").as_deref(), Some("0.167.0-alpha.2"));
}

#[test]
fn an_output_without_a_dotted_number_gives_its_first_word_and_a_blank_one_nothing() {
    assert_eq!(version_of("nightly build\n").as_deref(), Some("nightly"));
    assert_eq!(version_of("  \n"), None);
}

#[test]
fn capabilities_an_older_version_lacks_are_listed_with_the_version_that_adds_them() {
    let env = TestEnv::new();
    let codex = env.spec("codex");
    assert_eq!(missing_capabilities(&codex, Some("0.150.1")), IndexMap::from([(Capability::Usage, "0.164.0".to_string())]));
    assert_eq!(missing_capabilities(&codex, Some("0.164.0")), IndexMap::<Capability, String>::new());
    assert_eq!(missing_capabilities(&codex, Some("0.1000.0")), IndexMap::<Capability, String>::new(), "compared as numbers, not text");
}

#[test]
fn an_unknown_version_is_given_the_benefit_of_the_doubt() {
    let env = TestEnv::new();
    let codex = env.spec("codex");
    assert_eq!(missing_capabilities(&codex, None), IndexMap::<Capability, String>::new());
    assert_eq!(missing_capabilities(&codex, Some("nightly")), IndexMap::<Capability, String>::new());
}

/// Answers each run with the next output of a script (the out-of-process edge).
struct Script(Mutex<Vec<Option<&'static str>>>);

impl Script {
    fn new(outputs: Vec<Option<&'static str>>) -> Arc<Self> {
        Arc::new(Script(Mutex::new(outputs.into_iter().rev().collect())))
    }
}

impl CommandRunner for Script {
    fn run(&self, _cmd: &[String], _timeout: Duration) -> Option<String> {
        self.0.lock().unwrap().pop().expect("ran more often than scripted").map(String::from)
    }
}

/// A clock moved by hand.
struct ManualClock {
    start: Instant,
    offset: Mutex<Duration>,
}

impl ManualClock {
    fn new() -> Arc<Self> {
        Arc::new(ManualClock { start: Instant::now(), offset: Mutex::new(Duration::ZERO) })
    }
    fn advance(&self, by: Duration) {
        *self.offset.lock().unwrap() += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.start + *self.offset.lock().unwrap()
    }
}

#[test]
fn the_tool_version_is_remembered_until_it_is_older_than_max_age() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let clock = ManualClock::new();
    let cache = VersionCache::new(
        Script::new(vec![Some("2.1.316 (Claude Code)"), Some("2.1.317 (Claude Code)")]),
        clock.clone(),
        Duration::from_secs(300),
    );

    assert_eq!(cache.tool_version(&spec).as_deref(), Some("2.1.316"));
    clock.advance(Duration::from_secs(299));
    assert_eq!(cache.tool_version(&spec).as_deref(), Some("2.1.316"), "still fresh");
    clock.advance(Duration::from_secs(1));
    assert_eq!(cache.tool_version(&spec).as_deref(), Some("2.1.317"), "expired: run again");
}

#[test]
fn a_failed_version_run_gives_no_version_and_is_remembered_too() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code");
    let cache = VersionCache::new(Script::new(vec![None]), ManualClock::new(), Duration::from_secs(300));
    assert_eq!(cache.tool_version(&spec), None);
    assert_eq!(cache.tool_version(&spec), None); // a second run would exhaust the script and panic
}

#[test]
fn a_tool_without_a_version_command_has_no_version_and_runs_nothing() {
    let env = TestEnv::new();
    let cursor = env.spec("cursor");
    assert!(cursor.version_cmd.is_empty(), "precondition: cursor.yaml has no version_cmd");
    let cache = VersionCache::new(Script::new(vec![]), ManualClock::new(), Duration::from_secs(300));
    assert_eq!(cache.tool_version(&cursor), None);
}

#[test]
fn tools_sharing_a_version_command_share_the_cached_version() {
    let env = TestEnv::new();
    let cache = VersionCache::new(Script::new(vec![Some("2.1.316")]), ManualClock::new(), Duration::from_secs(300));
    assert_eq!(cache.tool_version(&env.spec("claude-code")).as_deref(), Some("2.1.316"));
    assert_eq!(cache.tool_version(&env.spec("claude-code-otel")).as_deref(), Some("2.1.316"));
}

// --- policy ----------------------------------------------------------------------------------

#[test]
fn a_user_setting_that_turns_hooks_off_blocks_connecting() {
    let env = TestEnv::new();
    let file = env.claude_settings();
    write_json(&file, &json!({"disableAllHooks": true}));
    let report = check_policy(&env.spec("claude-code"), &env.settings);
    assert_eq!(report, PolicyReport { blocks: vec![format!("disableAllHooks is set in {}", file.display())], warnings: vec![] });
}

#[test]
fn a_listed_value_inside_a_list_setting_blocks_connecting() {
    let env = TestEnv::new();
    write_json(&env.claude_settings(), &json!({"strictPluginOnlyCustomization": ["skills", "hooks"]}));
    assert_eq!(check_policy(&env.spec("claude-code"), &env.settings).blocks.len(), 1);
}

#[test]
fn a_setting_with_an_unlisted_value_does_not_block() {
    let env = TestEnv::new();
    write_json(&env.claude_settings(), &json!({"disableAllHooks": false, "strictPluginOnlyCustomization": ["skills"]}));
    assert_eq!(check_policy(&env.spec("claude-code"), &env.settings), PolicyReport::default());
}

#[test]
fn managed_settings_files_are_found_by_glob_in_sorted_order() {
    let env = TestEnv::new();
    let mut spec = env.spec("claude-code");
    let dir = env.path("managed-settings.d");
    spec.policy.files = vec![format!("{}/*.json", dir.display())];
    write_json(&dir.join("20-hooks.json"), &json!({"disableAllHooks": true}));
    write_json(&dir.join("10-managed.json"), &json!({"allowManagedHooksOnly": true}));
    write_json(&dir.join("30-other.json"), &json!({"theme": "dark"}));
    write_text(&dir.join("notes.txt"), "{\"disableAllHooks\": true}");

    let report = check_policy(&spec, &env.settings);

    assert_eq!(
        report.blocks,
        vec![
            format!("allowManagedHooksOnly is set in {}", dir.join("10-managed.json").display()),
            format!("disableAllHooks is set in {}", dir.join("20-hooks.json").display()),
        ]
    );
}

#[test]
fn a_settings_file_that_is_invalid_or_not_an_object_is_skipped() {
    let env = TestEnv::new();
    let mut spec = env.spec("claude-code");
    let dir = env.path("managed");
    spec.policy.files = vec![format!("{}/*.json", dir.display())];
    write_text(&dir.join("broken.json"), "{\"disableAllHooks\": true");
    write_text(&dir.join("list.json"), "[{\"disableAllHooks\": true}]");
    assert_eq!(check_policy(&spec, &env.settings), PolicyReport::default());
}

#[test]
fn an_existing_telemetry_endpoint_blocks_connecting_but_our_own_does_not() {
    let env = TestEnv::new();
    let spec = env.spec("claude-code-otel");
    let file = env.claude_settings();
    write_json(&file, &json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://collector.example.com"}}));
    assert_eq!(
        check_policy(&spec, &env.settings).blocks,
        vec![format!("env.OTEL_EXPORTER_OTLP_ENDPOINT is already set in {}", file.display())]
    );

    write_json(&file, &json!({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:7878/ingest/claude-code-otel"}}));
    assert_eq!(check_policy(&spec, &env.settings).blocks, Vec::<String>::new());
}

#[test]
fn a_warn_if_set_setting_warns_that_it_must_allow_the_receiver() {
    let env = TestEnv::new();
    let mut spec = env.spec("claude-code");
    spec.policy.warn_if_set = vec!["allowedHttpHookUrls".into()];
    let file = env.claude_settings();
    write_json(&file, &json!({"allowedHttpHookUrls": []}));
    assert_eq!(
        check_policy(&spec, &env.settings).warnings,
        vec![format!("allowedHttpHookUrls is set in {}: it must allow http://127.0.0.1:7878", file.display())]
    );
}

#[test]
fn a_telemetry_variable_in_the_environment_is_a_warning_not_a_block() {
    let env = TestEnv::with_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "https://collector.example.com")]);
    let report = check_policy(&env.spec("claude-code-otel"), &env.settings);
    assert_eq!(report.blocks, Vec::<String>::new());
    assert_eq!(
        report.warnings,
        vec![
            "OTEL_EXPORTER_OTLP_ENDPOINT is set in this shell's environment: where it is also set, it wins over the settings file"
                .to_string()
        ]
    );
}

#[test]
fn toml_settings_that_turn_codex_hooks_off_block_connecting() {
    let env = TestEnv::with_env(&[]);
    let home = env.path("codex-home");
    let mut settings = env.settings.clone();
    settings.env.set("CODEX_HOME", home.to_string_lossy());
    let spec = env.spec("codex");

    write_text(&home.join("config.toml"), "[features]\nhooks = false\n");
    assert_eq!(check_policy(&spec, &settings).blocks, vec![format!("features.hooks is set in {}", home.join("config.toml").display())]);

    write_text(&home.join("config.toml"), "[features]\ncodex_hooks = false\n");
    assert_eq!(check_policy(&spec, &settings).blocks.len(), 1, "the deprecated alias blocks too");
}

#[test]
fn unrelated_or_broken_codex_toml_does_not_block() {
    let env = TestEnv::with_env(&[]);
    let home = env.path("codex-home");
    let mut settings = env.settings.clone();
    settings.env.set("CODEX_HOME", home.to_string_lossy());
    let spec = env.spec("codex");

    write_text(&home.join("config.toml"), "model = \"gpt-5-codex\"\n[features]\nhooks = true\n");
    assert_eq!(check_policy(&spec, &settings), PolicyReport::default());
    write_text(&home.join("config.toml"), "[features\nhooks = false\n");
    assert_eq!(check_policy(&spec, &settings), PolicyReport::default());
}
