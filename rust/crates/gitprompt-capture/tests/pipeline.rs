//! The capture pipeline: decode, discard, redact, map, keep, then a prompt (with machine context) or
//! an event (clipped, joined to its prompt). Real specs, a real in-memory store, a stubbed machine.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use gitprompt_capture::pipeline::{CaptureOptions, Captured, Notice, Observer, StoredRow, capture, decode, join_session, observe};
use gitprompt_core::json::CLIP_LIMIT;
use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::{Format, bundled_defaults, parse_spec};
use gitprompt_core::{Json, JsonMap};
use pretty_assertions::assert_eq;
use serde_json::json;

fn hook_prompt() -> Json {
    json!({
        "session_id": "abc123",
        "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": "/work/app",
        "permission_mode": "default",
        "hook_event_name": "UserPromptSubmit",
        "prompt": "Write a function to calculate the factorial of a number"
    })
}

fn with(base: &Json, extra: Json) -> Json {
    let mut out = base.as_object().unwrap().clone();
    out.extend(obj(extra));
    Json::Object(out)
}

fn copilot(event: &str, session: &str, extra: Json) -> Json {
    with(&json!({"hook_event_name": event, "session_id": session, "cwd": "/work/app"}), extra)
}

fn otlp_log(event: &str, prompt_id: &str) -> Json {
    json!({"resourceLogs": [{"resource": {"attributes": [{"key": "service.name", "value": {"stringValue": "claude-code"}}]},
    "scopeLogs": [{"logRecords": [{"attributes": [
        {"key": "event.name", "value": {"stringValue": event}},
        {"key": "prompt.id", "value": {"stringValue": prompt_id}},
        {"key": "session.id", "value": {"stringValue": "abc123"}},
        {"key": "input_tokens", "value": {"intValue": "120"}}
    ]}]}]}]})
}

fn prompt_id_of(store: &gitprompt_store::Store, captured: &Captured) -> i64 {
    match only_row(captured) {
        StoredRow::Prompt(id) => {
            prompt_row(store, id); // panics if the prompt isn't actually in the store
            id
        }
        other => panic!("stored {other:?}, expected a prompt"),
    }
}

// --- prompts ---------------------------------------------------------------------------------

#[test]
fn a_prompt_event_is_stored_as_a_prompt_with_its_mapped_fields_and_raw_payload() {
    let env = TestEnv::new();
    let store = store();
    let captured = ingest(&env.spec("claude-code"), &store, &hook_prompt());

    let row = prompt_row(&store, prompt_id_of(&store, &captured));
    assert_eq!(row.source, "claude-code");
    assert_eq!(row.prompt.as_deref(), Some("Write a function to calculate the factorial of a number"));
    assert_eq!(row.fields["session_id"], json!("abc123"));
    assert_eq!(row.fields["cwd"], json!("/work/app"));
    assert_eq!(row.fields["event"], json!("UserPromptSubmit"));
    assert_eq!(row.raw, hook_prompt());
    assert_eq!((captured.prompts, captured.events), (1, 0));
}

#[test]
fn a_prompt_is_stamped_with_the_git_state_of_its_cwd_and_the_tool_version() {
    let env = TestEnv::new();
    let store = store();
    let repo = obj(json!({"git_root": "/work/app", "git_branch": "main"}));
    let git = GitByDir(BTreeMap::from([(PathBuf::from("/work/app"), repo)]));
    let versions = FixedVersion(Some("2.1.316"));
    let observer = Observer { git: &git, versions: &versions };

    let captured = capture(&env.spec("claude-code"), &store, &hook_prompt(), CaptureOptions::default(), &observer).unwrap();

    let row = prompt_row(&store, prompt_id_of(&store, &captured));
    assert_eq!(Json::Object(row.context), json!({"git_root": "/work/app", "git_branch": "main", "tool_version": "2.1.316"}));
}

#[test]
fn observe_leaves_out_the_tool_version_when_the_tool_reports_none() {
    let env = TestEnv::new();
    let repo = obj(json!({"git_root": "/r"}));
    let git = GitByDir(BTreeMap::from([(PathBuf::from("/r"), repo)]));
    let observer = Observer { git: &git, versions: &NO_VERSION };
    assert_eq!(Json::Object(observe(&env.spec("claude-code"), Some("/r"), &observer)), json!({"git_root": "/r"}));
    assert_eq!(observe(&env.spec("claude-code"), None, &observer), JsonMap::new());
}

#[test]
fn a_cursor_prompt_is_observed_in_the_first_folder_of_its_workspace() {
    let env = TestEnv::new();
    let store = store();
    let shop = obj(json!({"git_root": "/work/shop"}));
    let other = obj(json!({"git_root": "/work/shared-lib"}));
    let git = GitByDir(BTreeMap::from([(PathBuf::from("/work/shop"), shop), (PathBuf::from("/work/shared-lib"), other)]));
    let observer = Observer { git: &git, versions: &NO_VERSION };
    let payload = json!({"hook_event_name": "beforeSubmitPrompt", "conversation_id": "c1", "generation_id": "g1",
                         "workspace_roots": ["/work/shop", "/work/shared-lib"], "prompt": "rename the cart"});

    let captured = capture(&env.spec("cursor"), &store, &payload, CaptureOptions::default(), &observer).unwrap();

    let row = prompt_row(&store, prompt_id_of(&store, &captured));
    assert_eq!(row.context["git_root"], json!("/work/shop"));
    assert_eq!(row.fields["workspace_roots"], json!(["/work/shop", "/work/shared-lib"]));
}

#[test]
fn a_repeated_prompt_id_of_the_same_source_is_stored_once() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("claude-code");
    ingest(&spec, &store, &hook_prompt());

    let again = ingest(&spec, &store, &hook_prompt());

    assert_eq!((again.prompts, again.rows.clone()), (0, vec![]));
    assert_eq!(all_prompts(&store).len(), 1);
}

#[test]
fn a_long_prompt_is_stored_whole() {
    let env = TestEnv::new();
    let store = store();
    let text = "x".repeat(CLIP_LIMIT * 2);
    let captured = ingest(&env.spec("claude-code"), &store, &with(&hook_prompt(), json!({"prompt": text})));
    assert_eq!(prompt_row(&store, prompt_id_of(&store, &captured)).raw["prompt"], json!(text));
}

// --- events ----------------------------------------------------------------------------------

#[test]
fn an_other_event_is_stored_as_an_event_and_marks_the_turn_it_belongs_to() {
    let env = TestEnv::new();
    let store = store();
    let stop = with(&hook_prompt(), json!({"hook_event_name": "Stop", "last_assistant_message": "Done"}));

    let captured = ingest(&env.spec("claude-code"), &store, &stop);

    let StoredRow::Event(id) = only_row(&captured) else { panic!("not an event: {:?}", captured.rows) };
    let row = event_row(&store, id);
    assert_eq!((row.event.as_deref(), row.session_id.as_deref()), (Some("Stop"), Some("abc123")));
    assert_eq!(row.prompt_id.as_deref(), Some("550e8400-e29b-41d4-a716-446655440000"));
    assert_eq!(captured.turns.into_iter().collect::<Vec<_>>(), vec!["550e8400-e29b-41d4-a716-446655440000".to_string()]);
    assert_eq!((captured.prompts, captured.events), (0, 1));
}

#[test]
fn long_strings_in_an_event_are_clipped_before_storing() {
    let env = TestEnv::new();
    let store = store();
    let tool = with(
        &hook_prompt(),
        json!({"hook_event_name": "PostToolUse", "tool_name": "Write",
                                            "tool_input": {"file_path": "/r/a.py", "content": "x".repeat(20_000)}}),
    );

    let captured = ingest(&env.spec("claude-code"), &store, &tool);

    let StoredRow::Event(id) = only_row(&captured) else { panic!("not an event") };
    let raw = event_row(&store, id).raw;
    let expected = format!("{}… [10000 more characters not stored]", "x".repeat(10_000));
    assert_eq!(raw["tool_input"]["content"], json!(expected));
    assert_eq!(raw["tool_input"]["file_path"], json!("/r/a.py"));
}

// --- what the spec keeps ---------------------------------------------------------------------

#[test]
fn telemetry_events_the_spec_drops_are_not_stored_by_exact_name_or_prefix() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("claude-code-otel");

    let dropped = ingest(&spec, &store, &otlp_log("hook_registered", "p1"));
    let prefixed = ingest(&spec, &store, &otlp_log("hook_execution_start", "p1"));
    let kept = ingest(&spec, &store, &otlp_log("api_request", "p1"));

    assert!(dropped.is_empty() && prefixed.is_empty(), "dropped {dropped:?} / {prefixed:?}");
    assert_eq!(kept.events, 1);
    assert_eq!(all_events(&store).into_iter().map(|e| e.event).collect::<Vec<_>>(), vec![Some("api_request".to_string())]);
}

#[test]
fn a_spec_with_accept_events_stores_only_those_and_nothing_without_an_event_name() {
    let yaml = "id: picky\nname: Picky\ndetect: []\ninstall: {file: x.json, merge: {}}\naccept_events: [UserPromptSubmit, Stop]\n";
    let spec = parse_spec("picky.yaml", yaml, &bundled_defaults()).unwrap();
    let store = store();

    let stop = ingest(&spec, &store, &json!({"hook_event_name": "Stop", "session_id": "s"}));
    let tool = ingest(&spec, &store, &json!({"hook_event_name": "PostToolUse", "session_id": "s"}));
    let nameless = ingest(&spec, &store, &json!({"session_id": "s"}));

    assert_eq!((stop.events, tool.events, nameless.events), (1, 0, 0));
}

#[test]
fn an_event_without_a_name_is_kept_when_the_spec_accepts_every_event() {
    let env = TestEnv::new();
    let store = store();
    let captured = ingest(&env.spec("claude-code"), &store, &json!({"session_id": "s", "note": "no event name"}));
    assert_eq!(captured.events, 1);
}

// --- privacy ---------------------------------------------------------------------------------

#[test]
fn keys_the_spec_discards_are_removed_before_the_payload_is_stored_or_mapped() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("cursor");
    let prompt = json!({"hook_event_name": "beforeSubmitPrompt", "conversation_id": "c1", "generation_id": "g1",
                        "user_email": "someone@example.test", "prompt": "hi"});
    let stop = json!({"hook_event_name": "stop", "conversation_id": "c1", "generation_id": "g1",
                      "status": "completed", "user_email": "someone@example.test"});

    let p = ingest(&spec, &store, &prompt);
    let e = ingest(&spec, &store, &stop);

    let prompt_row = prompt_row(&store, prompt_id_of(&store, &p));
    let StoredRow::Event(event_id) = only_row(&e) else { panic!("not an event") };
    let stored = serde_json::to_string(&(prompt_row.raw, prompt_row.fields, event_row(&store, event_id).raw)).unwrap();
    assert!(!stored.contains("user_email") && !stored.contains("@example.test"), "stored: {stored}");
}

#[test]
fn hide_secrets_replaces_detected_secrets_in_the_prompt_and_raw_payload_before_storing() {
    let env = TestEnv::new();
    let store = store();
    let secret = format!("AKIA{}", "ABCDEFGHIJKLMNOP");
    let payload = with(&hook_prompt(), json!({"prompt": format!("use {secret}")}));
    let options = CaptureOptions { hide_secrets: true, ..CaptureOptions::default() };

    let captured = capture(&env.spec("claude-code"), &store, &payload, options, &blank()).unwrap();

    let row = prompt_row(&store, prompt_id_of(&store, &captured));
    assert_eq!(row.prompt.as_deref(), Some("use [hidden AWS access key]"));
    assert!(!serde_json::to_string(&(row.raw, row.fields)).unwrap().contains(&secret));
}

#[test]
fn hide_secrets_also_covers_events() {
    let env = TestEnv::new();
    let store = store();
    let tool = with(
        &hook_prompt(),
        json!({"hook_event_name": "PostToolUse", "tool_input": {"command": "export TOKEN=sk-ant-0123456789abcdefghijkl"}}),
    );
    let options = CaptureOptions { hide_secrets: true, ..CaptureOptions::default() };

    let captured = capture(&env.spec("claude-code"), &store, &tool, options, &blank()).unwrap();

    let StoredRow::Event(id) = only_row(&captured) else { panic!("not an event") };
    assert!(!event_row(&store, id).raw.to_string().contains("sk-ant-0123456789abcdefghijkl"));
}

#[test]
fn without_hide_secrets_the_prompt_is_stored_as_typed() {
    let env = TestEnv::new();
    let store = store();
    let secret = format!("AKIA{}", "ABCDEFGHIJKLMNOP");
    let captured = ingest(&env.spec("claude-code"), &store, &with(&hook_prompt(), json!({"prompt": format!("use {secret}")})));
    assert_eq!(prompt_row(&store, prompt_id_of(&store, &captured)).prompt, Some(format!("use {secret}")));
}

// --- turned off ------------------------------------------------------------------------------

#[test]
fn a_disabled_source_discards_everything_it_sends() {
    let env = TestEnv::new();
    let store = store();
    let options = CaptureOptions { enabled: false, ..CaptureOptions::default() };
    let spec = env.spec("claude-code");

    let prompt = capture(&spec, &store, &hook_prompt(), options, &blank()).unwrap();
    let stop = capture(&spec, &store, &with(&hook_prompt(), json!({"hook_event_name": "Stop"})), options, &blank()).unwrap();

    assert!(prompt.is_empty() && stop.is_empty());
    assert_eq!((all_prompts(&store).len(), all_events(&store).len()), (0, 0));
}

#[test]
fn options_follow_the_disabled_sources_and_hide_secrets_preferences() {
    let prefs = Preferences { disabled_sources: vec!["copilot-cli".into()], hide_secrets_on_ingest: true, ..Preferences::default() };
    assert_eq!(CaptureOptions::for_source(&prefs, "copilot-cli"), CaptureOptions { enabled: false, hide_secrets: true });
    assert_eq!(CaptureOptions::for_source(&prefs, "claude-code"), CaptureOptions { enabled: true, hide_secrets: true });
    assert_eq!(CaptureOptions::for_source(&Preferences::default(), "x"), CaptureOptions { enabled: true, hide_secrets: false });
}

// --- join: session ---------------------------------------------------------------------------

#[test]
fn a_session_joined_prompt_is_named_after_its_source_and_row_id() {
    let env = TestEnv::new();
    let store = store();
    let captured = ingest(&env.spec("copilot-cli"), &store, &copilot("UserPromptSubmit", "s1", json!({"prompt": "Add a retry"})));
    let id = prompt_id_of(&store, &captured);
    assert_eq!(prompt_row(&store, id).fields["prompt_id"], json!(format!("copilot-cli#{id}")));
}

#[test]
fn session_joined_events_take_the_latest_prompt_of_their_own_session() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("copilot-cli");
    let first = prompt_id_of(&store, &ingest(&spec, &store, &copilot("UserPromptSubmit", "s1", json!({"prompt": "one"}))));
    let tool = ingest(&spec, &store, &copilot("PostToolUse", "s1", json!({"tool_name": "bash"})));
    let second = prompt_id_of(&store, &ingest(&spec, &store, &copilot("UserPromptSubmit", "s1", json!({"prompt": "two"}))));
    let stop = ingest(&spec, &store, &copilot("Stop", "s1", json!({})));
    let elsewhere = ingest(&spec, &store, &copilot("Stop", "s2", json!({})));

    let joined = |c: &Captured| match only_row(c) {
        StoredRow::Event(id) => event_row(&store, id).prompt_id,
        other => panic!("{other:?}"),
    };
    assert_eq!(joined(&tool), Some(format!("copilot-cli#{first}")));
    assert_eq!(joined(&stop), Some(format!("copilot-cli#{second}")));
    assert_eq!(joined(&elsewhere), None);
    assert_eq!(stop.turns.into_iter().collect::<Vec<_>>(), vec![format!("copilot-cli#{second}")]);
}

#[test]
fn session_joined_prompts_with_the_same_text_are_both_stored() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("copilot-cli");
    let payload = copilot("UserPromptSubmit", "s1", json!({"prompt": "again"}));
    ingest(&spec, &store, &payload);
    assert_eq!(ingest(&spec, &store, &payload).prompts, 1);
}

#[test]
fn join_session_leaves_a_prompt_id_source_alone() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("claude-code");
    ingest(&spec, &store, &hook_prompt());
    let fields = obj(json!({"event": "Stop", "session_id": "abc123"}));
    assert_eq!(join_session(&spec, &store, fields.clone()).unwrap(), fields);
}

#[test]
fn join_session_keeps_an_id_the_event_already_has() {
    let env = TestEnv::new();
    let store = store();
    let spec = env.spec("copilot-cli");
    ingest(&spec, &store, &copilot("UserPromptSubmit", "s1", json!({"prompt": "one"})));
    let fields = obj(json!({"event": "Stop", "session_id": "s1", "prompt_id": "given"}));
    assert_eq!(join_session(&spec, &store, fields.clone()).unwrap(), fields);
}

// --- OTLP through the pipeline ---------------------------------------------------------------

#[test]
fn every_record_of_a_telemetry_export_is_stored_as_an_event_joined_by_prompt_id() {
    let env = TestEnv::new();
    let store = store();
    let captured = ingest(&env.spec("claude-code-otel"), &store, &otlp_log("api_request", "p1"));

    let StoredRow::Event(id) = only_row(&captured) else { panic!("not an event") };
    let row = event_row(&store, id);
    assert_eq!(
        (row.event.as_deref(), row.prompt_id.as_deref(), row.session_id.as_deref()),
        (Some("api_request"), Some("p1"), Some("abc123"))
    );
    assert_eq!((row.raw["input_tokens"].clone(), row.raw["service.name"].clone()), (json!(120), json!("claude-code")));
}

#[test]
fn telemetry_metric_points_are_stored_as_events_named_after_the_metric() {
    let env = TestEnv::new();
    let store = store();
    let body = json!({"resourceMetrics": [{"scopeMetrics": [{"metrics": [
        {"name": "claude_code.token.usage", "unit": "tokens", "sum": {"dataPoints": [{"asInt": "10"}, {"asInt": "20"}]}}
    ]}]}]});

    let captured = ingest(&env.spec("claude-code-otel"), &store, &body);

    assert_eq!(captured.events, 2);
    let names: Vec<Option<String>> = all_events(&store).into_iter().map(|e| e.event).collect();
    assert_eq!(names, vec![Some("claude_code.token.usage".to_string()); 2]);
}

#[test]
fn a_json_body_is_one_payload_and_an_otlp_body_one_per_record() {
    let body = json!({"hook_event_name": "Stop"});
    assert_eq!(decode(Format::Json, &body), vec![body.clone()]);
    assert_eq!(decode(Format::Otlp, &otlp_log("api_request", "p1")).len(), 1);
}

// --- what a capture reports ------------------------------------------------------------------

#[test]
fn a_capture_reports_its_counts_as_a_notice() {
    let captured = Captured { prompts: 1, events: 2, ..Captured::default() };
    assert_eq!(captured.notice(), Notice { prompts: 1, events: 2 });
    assert!(!captured.is_empty());
    assert!(Captured::default().is_empty());
}
