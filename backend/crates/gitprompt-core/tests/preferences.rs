//! Preferences: env > file > defaults, validation, atomic writes (port of `tests/core/test_preferences.py`).
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

use std::path::Path;

use common::{entries, mode};
use gitprompt_core::preferences::{Applies, KEYS, PreferenceFile, Preferences, RangeId, Resolved, ValueSource, WeekStart, env_name};
use gitprompt_core::{Env, Error, Json, JsonMap};
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

fn prefs(dir: &TempDir) -> PreferenceFile {
    PreferenceFile::new(dir.path(), Env::default())
}

fn prefs_with_env(dir: &TempDir, env: &[(&str, &str)]) -> PreferenceFile {
    PreferenceFile::new(dir.path(), Env::from_pairs(env.iter().copied()))
}

fn changes(value: Json) -> JsonMap {
    match value {
        Json::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

fn resolved(file: &PreferenceFile, key: &str) -> Resolved {
    file.resolve().unwrap().into_iter().find(|r| r.key == key).unwrap()
}

fn file_json(path: &Path) -> Json {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn is_invalid(err: &Error) -> bool {
    matches!(err, Error::InvalidPreferences(_))
}

// --- names and defaults ---------------------------------------------------------------------

#[test]
fn env_names_are_the_prefixed_upper_case_key() {
    assert_eq!(env_name("week_start"), "GITPROMPT_WEEK_START");
    assert_eq!(env_name("event_retention_days"), "GITPROMPT_EVENT_RETENTION_DAYS");
}

#[test]
fn defaults_without_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    let all = file.resolve().unwrap();
    let expected: Vec<Resolved> = [
        ("default_range", json!("7d")),
        ("week_start", json!("monday")),
        ("event_retention_days", json!(null)),
        ("hide_secrets_on_ingest", json!(false)),
        ("disabled_sources", json!([])),
    ]
    .into_iter()
    .map(|(key, value)| Resolved {
        key: key.into(),
        value: value.clone(),
        default: value,
        source: ValueSource::Default,
        env: env_name(key),
        applies: Applies::Now,
        locked: false,
    })
    .collect();
    assert_eq!(all, expected);
    assert_eq!(file.current().unwrap(), Preferences::default());
    assert!(!file.path.exists());
}

#[test]
fn resolve_lists_every_key_in_declaration_order() {
    let dir = tempfile::tempdir().unwrap();
    let keys: Vec<String> = prefs(&dir).resolve().unwrap().into_iter().map(|r| r.key).collect();
    assert_eq!(keys, KEYS);
}

// --- precedence -----------------------------------------------------------------------------

#[test]
fn env_wins_over_file_wins_over_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("preferences.json"), r#"{"default_range": "30d", "week_start": "sunday"}"#).unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_DEFAULT_RANGE", "24h"), ("GITPROMPT_EVENT_RETENTION_DAYS", "14")]);
    let summary: Vec<(String, Json, ValueSource, bool)> =
        file.resolve().unwrap().into_iter().map(|r| (r.key, r.value, r.source, r.locked)).collect();
    let expected = vec![
        ("default_range".to_string(), json!("24h"), ValueSource::Env, true),
        ("week_start".to_string(), json!("sunday"), ValueSource::File, false),
        ("event_retention_days".to_string(), json!(14), ValueSource::Env, true), // parsed from text
        ("hide_secrets_on_ingest".to_string(), json!(false), ValueSource::Default, false),
        ("disabled_sources".to_string(), json!([]), ValueSource::Default, false),
    ];
    assert_eq!(summary, expected);
    let current = file.current().unwrap();
    assert_eq!((current.default_range, current.week_start, current.event_retention_days), (RangeId::Day, WeekStart::Sunday, Some(14)));
}

#[test]
fn an_env_value_reports_the_default_it_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let r = resolved(&prefs_with_env(&dir, &[("GITPROMPT_WEEK_START", "sunday")]), "week_start");
    assert_eq!((r.value, r.default), (json!("sunday"), json!("monday")));
}

#[test]
fn empty_env_var_does_not_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_WEEK_START", " ")]);
    assert_eq!(resolved(&file, "week_start").source, ValueSource::Default);
    assert_eq!(file.overrides(), JsonMap::new());
}

#[test]
fn overrides_are_the_trimmed_raw_strings_of_preference_variables_only() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(
        &dir,
        &[("GITPROMPT_EVENT_RETENTION_DAYS", " 30 "), ("GITPROMPT_HOME", "/elsewhere"), ("GITPROMPT_NOPE", "x"), ("WEEK_START", "sunday")],
    );
    assert_eq!(file.overrides(), changes(json!({"event_retention_days": "30"})));
}

#[test]
fn a_boolean_env_value_is_parsed() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_HIDE_SECRETS_ON_INGEST", "true")]);
    assert!(file.current().unwrap().hide_secrets_on_ingest);
}

#[test]
fn disabled_sources_env_is_a_comma_separated_list() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_DISABLED_SOURCES", "copilot-cli, claude-code-otel")]);
    assert_eq!(resolved(&file, "disabled_sources").value, json!(["copilot-cli", "claude-code-otel"]));
    assert_eq!(file.current().unwrap().disabled_sources, ["copilot-cli", "claude-code-otel"]);
}

#[test]
fn blank_items_of_a_comma_separated_list_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_DISABLED_SOURCES", ",cursor, ,codex,")]);
    assert_eq!(file.current().unwrap().disabled_sources, ["cursor", "codex"]);
}

#[test]
fn invalid_env_value_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_EVENT_RETENTION_DAYS", "forever")]);
    let err = file.resolve().unwrap_err();
    assert!(matches!(&err, Error::InvalidPreferences(m) if m.contains("environment variable")), "{err:?}");
    assert!(is_invalid(&file.current().unwrap_err()));
}

#[test]
fn an_invalid_env_value_falls_back_to_defaults_for_effective() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("preferences.json"), r#"{"week_start": "sunday"}"#).unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_DEFAULT_RANGE", "1y")]);
    assert_eq!(file.effective(), Preferences::default());
}

#[test]
fn effective_is_current_when_everything_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("preferences.json"), r#"{"week_start": "sunday", "event_retention_days": 90}"#).unwrap();
    let expected = Preferences { week_start: WeekStart::Sunday, event_retention_days: Some(90), ..Preferences::default() };
    assert_eq!(prefs(&dir).effective(), expected);
}

// --- reading the file -----------------------------------------------------------------------

#[test]
fn a_missing_or_empty_file_stores_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    assert_eq!(file.stored().unwrap(), JsonMap::new());
    std::fs::write(&file.path, "").unwrap();
    assert_eq!(file.stored().unwrap(), JsonMap::new());
}

#[test]
fn stored_returns_what_the_file_sets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("preferences.json"), r#"{"event_retention_days": 30, "disabled_sources": ["cursor"]}"#).unwrap();
    assert_eq!(prefs(&dir).stored().unwrap(), changes(json!({"event_retention_days": 30, "disabled_sources": ["cursor"]})));
}

#[test]
fn invalid_file_is_reported_and_effective_falls_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    std::fs::write(&file.path, "{oops").unwrap();
    let err = file.resolve().unwrap_err();
    assert!(matches!(&err, Error::InvalidPreferences(m) if m.contains("not valid JSON")), "{err:?}");
    assert!(is_invalid(&file.update(&changes(json!({"week_start": "sunday"}))).unwrap_err())); // never overwrite it
    assert_eq!(std::fs::read_to_string(&file.path).unwrap(), "{oops");
    assert_eq!(file.effective().default_range, RangeId::Week);
}

#[test]
fn a_file_that_is_not_an_object_is_invalid_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    std::fs::write(&file.path, "[1, 2]").unwrap();
    let err = file.stored().unwrap_err();
    let path = file.path.display().to_string();
    assert!(matches!(&err, Error::InvalidPreferences(m) if m.contains(&path)), "{err:?}");
}

#[test]
fn an_invalid_value_in_the_file_is_reported_with_the_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    std::fs::write(&file.path, r#"{"week_start": "tuesday"}"#).unwrap();
    let err = file.stored().unwrap_err();
    let path = file.path.display().to_string();
    assert!(matches!(&err, Error::InvalidPreferences(m) if m.contains(&path)), "{err:?}");
}

#[test]
fn an_unknown_key_in_the_file_is_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    std::fs::write(&file.path, r#"{"theme": "dark"}"#).unwrap();
    assert!(is_invalid(&file.stored().unwrap_err()));
}

// --- writing --------------------------------------------------------------------------------

#[test]
fn update_writes_the_file_and_reports_changes() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    let changed = file.update(&changes(json!({"week_start": "sunday", "event_retention_days": 30}))).unwrap();
    assert_eq!(changed, ["week_start", "event_retention_days"]);
    assert_eq!(file_json(&file.path), json!({"event_retention_days": 30, "week_start": "sunday"}));
    assert_eq!(mode(&file.path), 0o600);
    assert_eq!(file.update(&changes(json!({"week_start": "sunday"}))).unwrap(), Vec::<String>::new()); // same value
    assert_eq!(file.update(&changes(json!({"event_retention_days": null}))).unwrap(), ["event_retention_days"]); // reset
    assert_eq!(file_json(&file.path), json!({"week_start": "sunday"}));
}

#[test]
fn changed_keys_are_reported_in_declaration_order() {
    let dir = tempfile::tempdir().unwrap();
    let changed = prefs(&dir).update(&changes(json!({"disabled_sources": ["cursor"], "default_range": "all"}))).unwrap();
    assert_eq!(changed, ["default_range", "disabled_sources"]);
}

#[test]
fn the_file_is_written_sorted_and_indented_with_a_final_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    file.update(&changes(json!({"week_start": "sunday", "default_range": "30d"}))).unwrap();
    assert_eq!(std::fs::read_to_string(&file.path).unwrap(), "{\n  \"default_range\": \"30d\",\n  \"week_start\": \"sunday\"\n}\n");
}

#[test]
fn update_keeps_values_it_does_not_change() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    file.update(&changes(json!({"hide_secrets_on_ingest": true}))).unwrap();
    file.update(&changes(json!({"week_start": "sunday"}))).unwrap();
    assert_eq!(file_json(&file.path), json!({"hide_secrets_on_ingest": true, "week_start": "sunday"}));
}

#[test]
fn resetting_a_value_that_is_not_set_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(prefs(&dir).update(&changes(json!({"event_retention_days": null}))).unwrap(), Vec::<String>::new());
}

#[test]
fn a_comma_separated_disabled_sources_string_is_stored_as_a_list() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    assert_eq!(file.update(&changes(json!({"disabled_sources": "cursor, codex"}))).unwrap(), ["disabled_sources"]);
    assert_eq!(file_json(&file.path), json!({"disabled_sources": ["cursor", "codex"]}));
}

#[test]
fn retention_of_one_day_and_of_ten_years_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    file.update(&changes(json!({"event_retention_days": 1}))).unwrap();
    assert_eq!(file.current().unwrap().event_retention_days, Some(1));
    file.update(&changes(json!({"event_retention_days": 3650}))).unwrap();
    assert_eq!(file.current().unwrap().event_retention_days, Some(3650));
}

#[test]
fn update_creates_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let file = PreferenceFile::new(&dir.path().join("not-yet"), Env::default());
    file.update(&changes(json!({"week_start": "sunday"}))).unwrap();
    assert_eq!(file_json(&file.path), json!({"week_start": "sunday"}));
}

#[test]
fn a_write_leaves_no_temporary_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    file.update(&changes(json!({"week_start": "sunday"}))).unwrap();
    file.update(&changes(json!({"default_range": "30d"}))).unwrap();
    assert_eq!(entries(dir.path()), ["preferences.json"]);
}

fn assert_refused(change: Json) {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs(&dir);
    file.update(&changes(json!({"default_range": "30d"}))).unwrap();
    let before = std::fs::read_to_string(&file.path).unwrap();
    let err = file.update(&changes(change.clone())).unwrap_err();
    assert!(is_invalid(&err), "{change}: {err:?}");
    assert_eq!(err.status(), 422);
    assert_eq!(std::fs::read_to_string(&file.path).unwrap(), before, "{change}");
}

#[test]
fn an_unknown_range_is_refused_and_nothing_is_written() {
    assert_refused(json!({"default_range": "1y"}));
}

#[test]
fn a_retention_of_zero_days_is_refused() {
    assert_refused(json!({"event_retention_days": 0}));
}

#[test]
fn a_retention_above_ten_years_is_refused() {
    assert_refused(json!({"event_retention_days": 3651}));
}

#[test]
fn a_negative_retention_is_refused() {
    assert_refused(json!({"event_retention_days": -1}));
}

#[test]
fn a_week_start_of_the_wrong_type_is_refused() {
    assert_refused(json!({"week_start": 3}));
}

#[test]
fn an_unknown_key_is_refused_with_its_name() {
    assert_refused(json!({"nope": 1}));
    let dir = tempfile::tempdir().unwrap();
    let err = prefs(&dir).update(&changes(json!({"zeta": 1, "alpha": 2}))).unwrap_err();
    assert!(matches!(&err, Error::InvalidPreferences(m) if m.contains("alpha") && m.contains("zeta")), "{err:?}");
}

#[test]
fn a_valid_change_next_to_an_invalid_one_is_not_written_either() {
    assert_refused(json!({"week_start": "sunday", "event_retention_days": 0}));
}

#[test]
fn env_locked_values_cannot_be_changed() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_EVENT_RETENTION_DAYS", "7")]);
    let err = file.update(&changes(json!({"event_retention_days": 30, "week_start": "sunday"}))).unwrap_err();
    assert!(matches!(&err, Error::Locked(m) if m.contains("GITPROMPT_EVENT_RETENTION_DAYS")), "{err:?}");
    assert_eq!(err.status(), 409);
    assert!(!file.path.exists()); // refused as a whole
}

#[test]
fn keys_not_locked_by_env_can_still_be_changed() {
    let dir = tempfile::tempdir().unwrap();
    let file = prefs_with_env(&dir, &[("GITPROMPT_EVENT_RETENTION_DAYS", "7")]);
    assert_eq!(file.update(&changes(json!({"week_start": "sunday"}))).unwrap(), ["week_start"]);
    assert_eq!(resolved(&file, "event_retention_days").value, json!(7));
}
