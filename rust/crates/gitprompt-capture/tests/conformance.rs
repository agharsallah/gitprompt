//! Connector conformance kit (mapping part): every fixture in `testdata/fixtures/<tool>/` is replayed,
//! in file-name order, through the capture pipeline with the tool's bundled spec, and must be stored
//! as `expect.stored_as` with at least `expect.fields`. Origin, typed text and turn expectations are
//! checked by gitprompt-insights.

mod common;

use common::*;
use gitprompt_capture::pipeline::StoredRow;
use gitprompt_core::JsonMap;
use gitprompt_core::spec::load_specs;
use serde_json::Value;

/// Replays a tool's fixtures; panics naming the first fixture that doesn't conform.
fn assert_conforms(tool: &str) {
    let env = TestEnv::new();
    let spec = env.spec(tool);
    let store = store();
    let cases = fixtures(tool);
    assert!(!cases.is_empty(), "{tool}: no fixtures");

    for (name, case) in cases {
        let expect = &case["expect"];
        let captured = ingest(&spec, &store, &case["payload"]);
        let (stored_as, fields): (&str, Option<JsonMap>) = match captured.rows.as_slice() {
            [] => ("dropped", None),
            [StoredRow::Prompt(id)] => ("prompt", Some(prompt_row(&store, *id).fields)),
            [StoredRow::Event(id)] => ("event", Some(event_row(&store, *id).fields)),
            more => panic!("{tool}/{name}: one hook payload stored {} rows", more.len()),
        };
        assert_eq!(stored_as, expect["stored_as"].as_str().unwrap(), "{tool}/{name}: stored as {stored_as}");
        let Some(fields) = fields else { continue };
        let wanted = expect["fields"].as_object().cloned().unwrap_or_default();
        let actual: JsonMap = wanted.keys().map(|k| (k.clone(), fields.get(k).cloned().unwrap_or(Value::Null))).collect();
        assert_eq!(actual, wanted, "{tool}/{name}: mapped {fields:?}");
        if stored_as == "prompt" {
            let id = fields.get("prompt_id").and_then(Value::as_str).unwrap_or_default();
            assert!(!id.is_empty(), "{tool}/{name}: a prompt needs a prompt id (sent, or given by `join`)");
        }
    }
}

#[test]
fn claude_code_fixtures_are_stored_as_expected() {
    assert_conforms("claude-code");
}

#[test]
fn codex_fixtures_are_stored_as_expected() {
    assert_conforms("codex");
}

#[test]
fn copilot_cli_fixtures_are_stored_as_expected() {
    assert_conforms("copilot-cli");
}

#[test]
fn cursor_fixtures_are_stored_as_expected() {
    assert_conforms("cursor");
}

#[test]
fn every_fixture_folder_is_covered_by_a_test_above() {
    assert_eq!(fixture_tools(), vec!["claude-code", "codex", "copilot-cli", "cursor"]);
}

#[test]
fn every_main_bundled_connector_has_fixtures() {
    let env = TestEnv::new();
    let tools = fixture_tools();
    let missing: Vec<String> =
        load_specs(&env.settings).values().filter(|s| s.part_of.is_none()).map(|s| s.id.clone()).filter(|id| !tools.contains(id)).collect();
    assert_eq!(missing, Vec::<String>::new(), "connectors without conformance fixtures");
}

#[test]
fn the_cursor_account_email_is_never_stored() {
    let env = TestEnv::new();
    let spec = env.spec("cursor");
    let store = store();
    for (_, case) in fixtures("cursor") {
        ingest(&spec, &store, &case["payload"]);
    }
    let prompts: Vec<_> = all_prompts(&store).into_iter().map(|p| (p.raw, p.fields)).collect();
    let events: Vec<_> = all_events(&store).into_iter().map(|e| (e.raw, e.fields)).collect();
    assert!(!prompts.is_empty() && !events.is_empty(), "the cursor fixtures stored nothing");
    let dump = serde_json::to_string(&(prompts, events)).unwrap();
    assert!(!dump.contains("user_email") && !dump.contains("@example.test"), "an email was stored");
}
