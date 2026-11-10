//! Source specs: patterns, name matching, Spec behaviour, parse_spec merge rules and validation, and
//! every bundled spec (port of tests/sources/test_spec.py and sources/spec.py).

mod common;

use std::path::PathBuf;

use common::{MINIMAL, bundled};
use gitprompt_core::spec::{
    BUNDLED, Capability, DEFAULTS, EventName, Format, Join, Origin, Pattern, Spec, SurfaceInfo, SurfaceRef, ToolKind, UsageValue,
    bundled_defaults, by_name, expand, matches, parse_spec,
};
use gitprompt_core::{Env, Error, Json, JsonMap, Settings};
use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::json;

fn no_defaults() -> serde_norway::Mapping {
    serde_norway::Mapping::new()
}

fn parse(text: &str) -> gitprompt_core::Result<Spec> {
    parse_spec("mytool.yaml", text, &no_defaults())
}

/// A bundled spec parsed on the bundled defaults.
fn bundled_spec(id: &str) -> Spec {
    parse_spec(&format!("{id}.yaml"), bundled(&format!("{id}.yaml")), &bundled_defaults()).unwrap()
}

/// The `Error::Spec` message of a spec that doesn't parse.
fn spec_error(text: &str) -> String {
    match parse(text) {
        Err(Error::Spec(message)) => message,
        other => panic!("expected Error::Spec, got {other:?}"),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn table<T: Clone>(pairs: &[(&str, T)]) -> IndexMap<String, T> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn obj(value: Json) -> JsonMap {
    match value {
        Json::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

// --- Pattern --------------------------------------------------------------------------------

#[test]
fn a_pattern_keeps_its_source_text() {
    assert_eq!(Pattern::new(r"^\d+$").unwrap().as_str(), r"^\d+$");
}

#[test]
fn an_invalid_pattern_is_an_error() {
    assert!(Pattern::new("(unclosed").is_err());
}

#[test]
fn patterns_support_python_style_look_around() {
    let pattern = Pattern::new(r"sk-(?!ant-)\w+").unwrap();
    assert!(pattern.matches_start("sk-proj1"));
    assert!(!pattern.matches_start("sk-ant-1"));
    assert!(Pattern::new(r"(?<=@)\w+").unwrap().is_match("user@host"));
}

#[test]
fn matches_start_anchors_at_the_start_only() {
    let pattern = Pattern::new("ab").unwrap();
    assert!(pattern.matches_start("abc"));
    assert!(!pattern.matches_start("xabc"));
}

#[test]
fn matches_start_anchors_every_alternative() {
    assert!(!Pattern::new("x|b").unwrap().matches_start("ab"));
    assert!(Pattern::new("x|a").unwrap().matches_start("ab"));
}

#[test]
fn is_match_finds_the_pattern_anywhere() {
    let pattern = Pattern::new("b+").unwrap();
    assert!(pattern.is_match("aabba"));
    assert!(!pattern.is_match("aaa"));
}

#[test]
fn first_groups_lists_the_first_group_of_every_match() {
    let pattern = Pattern::new(r"(\w+)=(\d)").unwrap();
    assert_eq!(pattern.first_groups("a=1 b=2 c=x"), strings(&["a", "b"]));
    assert_eq!(pattern.first_groups("nothing"), Vec::<String>::new());
}

#[test]
fn patterns_compare_and_serialize_as_their_source() {
    assert_eq!(Pattern::new("a+").unwrap(), Pattern::new("a+").unwrap());
    assert_ne!(Pattern::new("a+").unwrap(), Pattern::new("a*").unwrap());
    assert_eq!(serde_json::to_value(Pattern::new("a+").unwrap()).unwrap(), json!("a+"));
    assert_eq!(serde_json::from_value::<Pattern>(json!("b?")).unwrap(), Pattern::new("b?").unwrap());
    assert!(serde_json::from_value::<Pattern>(json!("(")).is_err());
}

// --- matches / by_name ----------------------------------------------------------------------

#[test]
fn drop_patterns_match_exact_names_and_trailing_globs() {
    assert!(matches("hook_*", Some("hook_x")));
    assert!(matches("hook_*", Some("hook_")));
    assert!(!matches("hook_*", Some("a_hook_x")));
    assert!(!matches("hook", Some("hook_x")));
    assert!(matches("hook", Some("hook")));
}

#[test]
fn a_lone_star_matches_any_name_but_not_none() {
    assert!(matches("*", Some("anything")));
    assert!(matches("*", Some("")));
    assert!(!matches("*", None));
}

#[test]
fn a_star_inside_a_pattern_is_literal() {
    assert!(!matches("a*b", Some("axb")));
    assert!(matches("a*b", Some("a*b")));
}

#[test]
fn by_name_prefers_the_exact_name_over_an_earlier_prefix() {
    let kinds = table(&[("mcp__*", "prefix"), ("mcp__x", "exact")]);
    assert_eq!(by_name(&kinds, Some("mcp__x")), Some(&"exact"));
    assert_eq!(by_name(&kinds, Some("mcp__y")), Some(&"prefix"));
}

#[test]
fn by_name_takes_the_first_matching_prefix_in_table_order() {
    let kinds = table(&[("ab*", 1), ("a*", 2)]);
    assert_eq!(by_name(&kinds, Some("abc")), Some(&1));
    assert_eq!(by_name(&kinds, Some("axe")), Some(&2));
}

#[test]
fn by_name_finds_nothing_for_an_unknown_or_missing_name() {
    let kinds = table(&[("Read", 1), ("mcp__*", 2)]);
    assert_eq!(by_name(&kinds, Some("Write")), None);
    assert_eq!(by_name(&kinds, None), None);
}

// --- expand ---------------------------------------------------------------------------------

fn settings_with(env: &[(&str, &str)]) -> Settings {
    let mut settings = Settings::new("/gp", "/home/ann");
    settings.env = Env::from_pairs(env.iter().copied());
    settings
}

#[test]
fn expand_uses_env_default_and_user_home() {
    let path = "${CLAUDE_CONFIG_DIR:-~/.claude}/settings.json";
    assert_eq!(expand(path, &settings_with(&[])), PathBuf::from("/home/ann/.claude/settings.json"));
    assert_eq!(expand(path, &settings_with(&[("CLAUDE_CONFIG_DIR", "/elsewhere")])), PathBuf::from("/elsewhere/settings.json"));
}

#[test]
fn an_empty_variable_falls_back_to_its_default() {
    let settings = settings_with(&[("CODEX_HOME", "")]);
    assert_eq!(expand("${CODEX_HOME:-~/.codex}/hooks.json", &settings), PathBuf::from("/home/ann/.codex/hooks.json"));
}

#[test]
fn an_empty_default_expands_to_nothing() {
    assert_eq!(expand("${NOPE:-}/x", &settings_with(&[])), PathBuf::from("/x"));
}

#[test]
fn several_variables_in_one_path_are_all_expanded() {
    let settings = settings_with(&[("A", "/a")]);
    assert_eq!(expand("${A:-/none}/${B:-b}/c", &settings), PathBuf::from("/a/b/c"));
}

#[test]
fn a_lone_tilde_is_the_user_home() {
    assert_eq!(expand("~", &settings_with(&[])), PathBuf::from("/home/ann"));
}

#[test]
fn a_tilde_path_is_resolved_against_user_home_not_the_real_home() {
    assert_eq!(expand("~/.cursor/hooks.json", &settings_with(&[("HOME", "/real")])), PathBuf::from("/home/ann/.cursor/hooks.json"));
}

#[test]
fn an_absolute_path_is_kept() {
    let path = "/Library/Application Support/ClaudeCode/managed-settings.json";
    assert_eq!(expand(path, &settings_with(&[])), PathBuf::from(path));
}

#[test]
fn a_variable_without_a_default_is_left_as_written() {
    assert_eq!(expand("${HOME}/x", &settings_with(&[("HOME", "/h")])), PathBuf::from("${HOME}/x"));
}

// --- Spec methods ---------------------------------------------------------------------------

#[test]
fn drop_events_match_exact_names_and_trailing_globs() {
    let spec = bundled_spec("claude-code-otel");
    assert!(!spec.keeps(Some("hook_registered")) && !spec.keeps(Some("hook_execution_complete")));
    assert!(spec.keeps(Some("api_request")) && spec.keeps(Some("hook_registered_later")) && spec.keeps(None));
}

#[test]
fn accept_events_apply_before_drop_events() {
    let mut spec = bundled_spec("claude-code");
    spec.accept_events = strings(&["Stop", "Notification"]);
    spec.drop_events = table(&[("Sto*", "no".to_string())]);
    assert!(spec.keeps(Some("Notification")) && !spec.keeps(Some("Stop")) && !spec.keeps(Some("PostToolUse")));
}

#[test]
fn an_event_without_a_name_is_dropped_when_accept_events_is_set() {
    let mut spec = bundled_spec("claude-code");
    spec.accept_events = strings(&["Stop"]);
    assert!(!spec.keeps(None));
}

#[test]
fn a_spec_without_accept_or_drop_events_keeps_everything() {
    let spec = parse(MINIMAL).unwrap();
    assert!(spec.keeps(Some("Anything")) && spec.keeps(None));
}

#[test]
fn the_label_is_the_short_name_else_the_name() {
    assert_eq!(bundled_spec("codex").label(), "Codex");
    assert_eq!(bundled_spec("claude-code").label(), "Claude Code");
}

#[test]
fn the_marker_matches_this_source_and_not_one_whose_id_starts_the_same() {
    let marker = bundled_spec("claude-code").marker();
    assert!(marker.is_match("curl -s 'http://127.0.0.1:7878/ingest/claude-code' >/dev/null"));
    assert!(marker.is_match("http://127.0.0.1:7878/ingest/claude-code/v1/logs"));
    assert!(marker.is_match("http://127.0.0.1:7878/ingest/claude-code"));
    assert!(!marker.is_match("http://127.0.0.1:7878/ingest/claude-code-otel"));
    assert!(!marker.is_match("http://127.0.0.1:7878/ingest/claude-codex"));
}

#[test]
fn the_marker_treats_the_id_literally() {
    let spec = parse(&MINIMAL.replace("id: mytool", "id: my.tool")).unwrap();
    assert!(spec.marker().is_match("/ingest/my.tool"));
    assert!(!spec.marker().is_match("/ingest/myXtool"));
}

#[test]
fn tool_kinds_map_names_and_prefixes_and_unmapped_tools_are_other() {
    let spec = bundled_spec("claude-code");
    let kinds: Vec<ToolKind> = [Some("Read"), Some("Bash"), Some("mcp__github__search"), Some("Unknown"), None]
        .into_iter()
        .map(|tool| spec.tool_kind(tool))
        .collect();
    assert_eq!(kinds, [ToolKind::Read, ToolKind::Shell, ToolKind::Mcp, ToolKind::Other, ToolKind::Other]);
    assert_eq!(bundled_spec("codex").tool_kind(Some("collaborationspawn_agent")), ToolKind::Agent);
}

#[test]
fn surface_info_names_a_declared_surface() {
    let spec = bundled_spec("claude-code");
    let desktop = SurfaceRef { id: "desktop".into(), name: "Desktop app".into(), icon: "app-window".into() };
    assert_eq!(spec.surface_info(Some("desktop")), Some(desktop));
}

#[test]
fn surface_info_is_none_for_an_unknown_empty_or_missing_surface() {
    let spec = bundled_spec("claude-code");
    assert_eq!((spec.surface_info(Some("teletype")), spec.surface_info(Some("")), spec.surface_info(None)), (None, None, None));
}

#[test]
fn extract_maps_fields_and_renames_the_event() {
    let spec = bundled_spec("cursor");
    let payload = json!({"hook_event_name": "beforeSubmitPrompt", "conversation_id": "c1", "generation_id": "g1", "prompt": "hi", "workspace_roots": ["/w"]});
    let expected =
        json!({"event": "UserPromptSubmit", "prompt": "hi", "session_id": "c1", "prompt_id": "g1", "cwd": "/w", "workspace_roots": ["/w"]});
    assert_eq!(Json::Object(spec.extract(&payload)), expected);
}

#[test]
fn the_first_event_name_rule_that_applies_wins() {
    let spec = bundled_spec("cursor");
    let event = |status: &str| spec.extract(&json!({"hook_event_name": "stop", "status": status})).get("event").cloned();
    assert_eq!(
        [event("aborted"), event("error"), event("completed")],
        [Some(json!("Interrupt")), Some(json!("StopFailure")), Some(json!("Stop"))]
    );
}

#[test]
fn an_event_no_rule_renames_is_kept_and_a_payload_without_event_gets_none() {
    let spec = bundled_spec("cursor");
    assert_eq!(spec.extract(&json!({"hook_event_name": "workspaceOpen"})).get("event"), Some(&json!("workspaceOpen")));
    assert_eq!(spec.extract(&json!({"prompt": "hi"})), obj(json!({"prompt": "hi"})));
}

#[test]
fn an_event_name_rule_compares_equals_with_its_type() {
    let rule = EventName { from: "e".into(), to: "E".into(), path: Some("n".into()), equals: Some(json!(1)) };
    assert!(rule.applies(Some("e"), &json!({"n": 1})));
    assert!(!rule.applies(Some("e"), &json!({"n": "1"})));
    assert!(!rule.applies(Some("other"), &json!({"n": 1})));
    assert!(!rule.applies(None, &json!({"n": 1})));
}

#[test]
fn an_event_name_rule_without_a_path_applies_to_every_payload_of_its_event() {
    let rule = EventName { from: "stop".into(), to: "Stop".into(), path: None, equals: None };
    assert!(rule.applies(Some("stop"), &json!({})));
}

#[test]
fn discarding_removes_top_level_discard_keys_only() {
    let spec = bundled_spec("cursor");
    let payload = json!({"user_email": "a@b.c", "prompt": "hi", "nested": {"user_email": "kept"}});
    assert_eq!(spec.discarding(payload), json!({"prompt": "hi", "nested": {"user_email": "kept"}}));
}

#[test]
fn discarding_without_discard_keys_or_on_a_non_object_changes_nothing() {
    let payload = json!({"user_email": "a@b.c"});
    assert_eq!(bundled_spec("codex").discarding(payload.clone()), payload);
    assert_eq!(bundled_spec("cursor").discarding(json!(["user_email"])), json!(["user_email"]));
}

// --- parse_spec: defaults and merge rules ---------------------------------------------------

const DEFAULTS_YAML: &str = r#"
map: {prompt: [prompt, text], event: [hook_event_name]}
tool_kinds: {Read: read, Bash: shell}
origin:
  - {when: {field: agent_id}, origin: subagent}
surface:
  - {when: {path: ide}, surface: vscode}
injected: [system-reminder, shared]
usage: {events: [api_request], values: {model: model}}
answer: {Stop: last_assistant_message}
shown: {a: shared a, b: shared b}
ignored: {x: shared x}
surfaces: {cli: {name: CLI, icon: terminal}, vscode: {name: VS Code, icon: code}}
"#;

const OWN_YAML: &str = r#"
map: {prompt: [input], session_id: [sid]}
tool_kinds: {Bash: other, Edit: write}
origin:
  - {when: {text_prefix: "<x>"}, origin: task_notification}
surface:
  - {when: {path: app}, surface: desktop}
injected: [mine, system-reminder]
usage: {events: [PostModelRequest]}
answer: {AgentResponse: text}
shown: {b: own b, c: own c}
ignored: {y: own y}
surfaces: {desktop: {name: Desktop}}
"#;

fn defaults() -> serde_norway::Mapping {
    serde_norway::from_str(DEFAULTS_YAML).unwrap()
}

fn merged() -> Spec {
    parse_spec("mytool.yaml", &format!("{MINIMAL}{OWN_YAML}"), &defaults()).unwrap()
}

fn inherited() -> Spec {
    parse_spec("mytool.yaml", MINIMAL, &defaults()).unwrap()
}

#[test]
fn a_minimal_spec_gets_documented_defaults() {
    let spec = parse(MINIMAL).unwrap();
    assert_eq!((spec.id.as_str(), spec.name.as_str(), spec.part_of.clone()), ("mytool", "My tool", None));
    assert_eq!((spec.format, spec.join, spec.prompt_events.clone()), (Format::Json, Join::PromptId, strings(&["UserPromptSubmit"])));
    assert_eq!((spec.install.file.as_str(), spec.install.merge.clone()), ("~/.mytool/hooks.json", JsonMap::new()));
    assert_eq!(spec.detect, strings(&["~/.mytool"]));
}

#[test]
fn map_entries_replace_default_fields_one_by_one() {
    let expected = table(&[("prompt", strings(&["input"])), ("event", strings(&["hook_event_name"])), ("session_id", strings(&["sid"]))]);
    assert_eq!(merged().map, expected);
    assert_eq!(inherited().map, table(&[("prompt", strings(&["prompt", "text"])), ("event", strings(&["hook_event_name"]))]));
}

#[test]
fn tool_kinds_replace_defaults_per_tool_keeping_default_order_first() {
    let kinds: Vec<(String, ToolKind)> = merged().tool_kinds.into_iter().collect();
    let expected = vec![("Read".to_string(), ToolKind::Read), ("Bash".to_string(), ToolKind::Other), ("Edit".to_string(), ToolKind::Write)];
    assert_eq!(kinds, expected);
}

#[test]
fn own_origin_rules_come_before_the_shared_ones() {
    let origins: Vec<Origin> = merged().origin.iter().map(|r| r.origin).collect();
    assert_eq!(origins, [Origin::TaskNotification, Origin::Subagent]);
    assert_eq!(inherited().origin.iter().map(|r| r.origin).collect::<Vec<_>>(), [Origin::Subagent]);
}

#[test]
fn own_surface_rules_come_before_the_shared_ones() {
    let surfaces: Vec<String> = merged().surface.into_iter().map(|r| r.surface).collect();
    assert_eq!(surfaces, strings(&["desktop", "vscode"]));
}

#[test]
fn injected_tags_are_the_shared_ones_then_own_without_duplicates() {
    assert_eq!(merged().injected, strings(&["system-reminder", "shared", "mine"]));
    assert_eq!(inherited().injected, strings(&["system-reminder", "shared"]));
}

#[test]
fn usage_is_replaced_whole_by_a_spec_of_its_own() {
    let own = merged().usage;
    assert_eq!((own.events, own.values.len(), own.input_includes_cached), (strings(&["PostModelRequest"]), 0, false));
    let shared = inherited().usage;
    assert_eq!(
        (shared.events, shared.values),
        (strings(&["api_request"]), [(UsageValue::Model, "model".to_string())].into_iter().collect())
    );
}

#[test]
fn an_empty_usage_falls_back_to_the_shared_one() {
    let spec = parse_spec("mytool.yaml", &format!("{MINIMAL}usage: {{}}\n"), &defaults()).unwrap();
    assert_eq!(spec.usage.events, strings(&["api_request"]));
}

#[test]
fn answer_is_replaced_whole_by_a_spec_of_its_own() {
    assert_eq!(merged().answer, table(&[("AgentResponse", "text".to_string())]));
    assert_eq!(inherited().answer, table(&[("Stop", "last_assistant_message".to_string())]));
}

#[test]
fn shown_and_ignored_add_own_keys_to_the_shared_ones() {
    let spec = merged();
    let shown = table(&[("a", "shared a".to_string()), ("b", "own b".to_string()), ("c", "own c".to_string())]);
    assert_eq!(spec.shown, shown);
    assert_eq!(spec.ignored, table(&[("x", "shared x".to_string()), ("y", "own y".to_string())]));
}

#[test]
fn surfaces_add_to_the_shared_ones_with_a_default_icon() {
    let expected = table(&[
        ("cli", SurfaceInfo { name: "CLI".into(), icon: "terminal".into() }),
        ("vscode", SurfaceInfo { name: "VS Code".into(), icon: "code".into() }),
        ("desktop", SurfaceInfo { name: "Desktop".into(), icon: "app".into() }),
    ]);
    assert_eq!(merged().surfaces, expected);
}

#[test]
fn null_merged_keys_count_as_absent() {
    let spec = parse_spec("mytool.yaml", &format!("{MINIMAL}map:\norigin:\ninjected:\nshown:\n"), &defaults()).unwrap();
    assert_eq!(spec.map, inherited().map);
    assert_eq!(spec.origin.len(), 1);
    assert_eq!(spec.injected, strings(&["system-reminder", "shared"]));
    assert_eq!(spec.shown, inherited().shown);
}

#[test]
fn keys_without_a_merge_rule_are_the_spec_own() {
    let spec = parse_spec("mytool.yaml", &format!("{MINIMAL}capabilities: [prompt]\n"), &defaults()).unwrap();
    assert_eq!(spec.capabilities, [Capability::Prompt]);
    assert_eq!(inherited().capabilities, Vec::<Capability>::new());
}

#[test]
fn unknown_top_level_keys_such_as_yaml_anchors_are_allowed() {
    let text = format!("{MINIMAL}x-hook: &hook {{type: command}}\n");
    assert_eq!(parse(&text).unwrap().id, "mytool");
}

#[test]
fn surfaces_named_in_captures_may_be_declared_by_the_defaults() {
    let spec = parse_spec("mytool.yaml", &format!("{MINIMAL}captures: {{cli: In a terminal.}}\n"), &defaults()).unwrap();
    assert_eq!(spec.captures, table(&[("cli", "In a terminal.".to_string())]));
}

// --- parse_spec: errors ---------------------------------------------------------------------

#[test]
fn text_that_is_not_yaml_is_an_error_naming_the_file() {
    let message = spec_error("id: [unclosed\n");
    assert!(message.contains("mytool.yaml") && message.contains("is not valid YAML"), "{message}");
}

#[test]
fn yaml_that_is_not_a_mapping_is_an_error_naming_the_file() {
    let list = spec_error("- a list\n");
    let empty = spec_error("");
    assert!(list.contains("mytool.yaml") && list.contains("must be a YAML mapping"), "{list}");
    assert!(empty.contains("must be a YAML mapping"), "{empty}");
}

#[test]
fn spec_errors_answer_500() {
    assert_eq!(parse("id: mytool\n").unwrap_err().status(), 500);
}

/// (case, spec text) of specs that are YAML mappings but not valid specs.
fn invalid_specs() -> Vec<(&'static str, String)> {
    let with = |extra: &str| format!("{MINIMAL}{extra}\n");
    vec![
        ("missing keys", "id: mytool\n".to_string()),
        ("install without merge", "id: mytool\nname: x\ndetect: []\ninstall: {file: x}\n".to_string()),
        ("unknown capability", with("capabilities: [telepathy]")),
        ("unknown join", with("join: guess")),
        ("unknown tool kind", with("tool_kinds: {Run: teleport}")),
        ("unknown format", with("format: xml")),
        ("unknown origin", with("origin: [{when: {field: a}, origin: alien}]")),
        ("origin rule extra key", with("origin: [{when: {field: a}, origin: typed, why: x}]")),
        ("unknown usage value", with("usage: {values: {tokens: t}}")),
        ("unknown usage key", with("usage: {events: [x], extra: 1}")),
        ("capability_since unknown capability", with("capability_since: {telepathy: 1.0}")),
        ("invalid tool_files pattern", with("tool_files: {apply_patch: '(unclosed'}")),
        ("event_names unknown key", with("event_names: [{from: a, to: b, when: c}]")),
        ("event_names without to", with("event_names: [{from: a}]")),
        ("map that is a list", with("map: [prompt]")),
        ("when without condition", with("origin: [{when: {}, origin: typed}]")),
        ("when with only equals", with("origin: [{when: {equals: 1}, origin: typed}]")),
        ("equals without path", with("origin: [{when: {field: a, equals: 1}, origin: typed}]")),
        ("when unknown key", with("origin: [{when: {fields: a}, origin: typed}]")),
        ("when invalid regex", with("origin: [{when: {text_regex: '(x'}, origin: typed}]")),
        ("when equals a list", with("origin: [{when: {path: a, equals: [1]}, origin: typed}]")),
        ("surface rule naming an undeclared surface", with("surface: [{when: {path: a}, surface: nowhere}]")),
        ("captures naming an undeclared surface", with("captures: {nowhere: x}")),
        ("surface id with upper case", with("surfaces: {Desktop: {name: D}}")),
        ("surface id starting with a digit", with("surfaces: {1app: {name: D}}\ncaptures: {1app: x}")),
        ("surface info without name", with("surfaces: {app: {icon: x}}")),
        ("surface info extra key", with("surfaces: {app: {name: A, color: red}}")),
        ("injected tag starting with a digit", with("injected: [1abc]")),
        ("injected tag with a space", with("injected: [a b]")),
    ]
}

#[test]
fn invalid_specs_are_errors_naming_the_file() {
    let results: Vec<(&str, String)> = invalid_specs()
        .into_iter()
        .map(|(case, text)| {
            let verdict = match parse(&text) {
                Err(Error::Spec(m)) if m.contains("mytool.yaml") && m.contains("is invalid") => "invalid".to_string(),
                other => format!("{other:?}"),
            };
            (case, verdict)
        })
        .collect();
    let expected: Vec<(&str, String)> = invalid_specs().into_iter().map(|(case, _)| (case, "invalid".to_string())).collect();
    assert_eq!(results, expected);
}

/// (case, spec text) of edge cases that must parse.
fn valid_specs() -> Vec<(&'static str, String)> {
    let with = |extra: &str| format!("{MINIMAL}{extra}\n");
    vec![
        ("when with only an event", with("origin: [{when: {event: UserPromptExpansion}, origin: slash_command}]")),
        ("equals with a path", with("origin: [{when: {path: a.b, equals: sdk-cli}, origin: programmatic}]")),
        (
            "equals a number or a bool",
            with("origin: [{when: {path: n, equals: 1}, origin: typed}, {when: {path: b, equals: true}, origin: typed}]"),
        ),
        ("text_regex with look-ahead", with("origin: [{when: {text_regex: '/(?!x)\\w+'}, origin: slash_command}]")),
        ("surface id with digits and dashes", with("surfaces: {my-app2: {name: Mine}}\nsurface: [{when: {path: a}, surface: my-app2}]")),
        ("injected tags with dashes and colons", with("injected: [local-command-stdout, ns:tag, A_b]")),
        ("cannot_capture any name", with("cannot_capture: {Claude Desktop chat: no hooks}")),
    ]
}

#[test]
fn edge_cases_of_valid_specs_parse() {
    let results: Vec<(&str, String)> =
        valid_specs().into_iter().map(|(case, text)| (case, parse(&text).map(|s| s.id).unwrap_or_else(|e| e.to_string()))).collect();
    let expected: Vec<(&str, String)> = valid_specs().into_iter().map(|(case, _)| (case, "mytool".to_string())).collect();
    assert_eq!(results, expected);
}

#[test]
fn when_conditions_are_read_into_their_fields() {
    let text =
        format!("{MINIMAL}origin: [{{when: {{event: user_prompt, path: app.entrypoint, equals: sdk-cli}}, origin: programmatic}}]\n");
    let when = &parse(&text).unwrap().origin[0].when;
    assert_eq!(
        (when.event.as_deref(), when.path.as_deref(), when.equals.clone()),
        (Some("user_prompt"), Some("app.entrypoint"), Some(json!("sdk-cli")))
    );
    assert_eq!((when.field.clone(), when.text_prefix.clone(), when.text_regex.clone()), (None, None, None));
}

// --- bundled specs --------------------------------------------------------------------------

#[test]
fn bundled_files_are_the_defaults_and_five_specs() {
    let names: Vec<&str> = BUNDLED.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, [DEFAULTS, "claude-code-otel.yaml", "claude-code.yaml", "codex.yaml", "copilot-cli.yaml", "cursor.yaml"]);
}

#[test]
fn bundled_defaults_hold_the_shared_sections() {
    let keys: Vec<String> = bundled_defaults().keys().map(|k| k.as_str().unwrap().to_string()).collect();
    assert_eq!(keys, strings(&["map", "origin", "injected", "usage", "shown", "ignored", "answer", "surfaces"]));
}

#[test]
fn every_bundled_spec_parses_with_the_id_of_its_file() {
    let ids: Vec<(String, String)> = BUNDLED
        .iter()
        .filter(|(name, _)| *name != DEFAULTS)
        .map(|(name, text)| (name.to_string(), parse_spec(name, text, &bundled_defaults()).map(|s| s.id).unwrap_or_else(|e| e.to_string())))
        .collect();
    let expected: Vec<(String, String)> = ["claude-code-otel", "claude-code", "codex", "copilot-cli", "cursor"]
        .into_iter()
        .map(|id| (format!("{id}.yaml"), id.to_string()))
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn every_bundled_spec_declares_what_it_yields() {
    let empty: Vec<&str> = ["claude-code-otel", "claude-code", "codex", "copilot-cli", "cursor"]
        .into_iter()
        .filter(|id| bundled_spec(id).capabilities.is_empty())
        .collect();
    assert_eq!(empty, Vec::<&str>::new());
}

#[test]
fn layers_name_their_main_connector() {
    let (hooks, telemetry) = (bundled_spec("claude-code"), bundled_spec("claude-code-otel"));
    assert_eq!((hooks.part_of, hooks.layer.as_str()), (None, "Hooks"));
    assert_eq!((telemetry.part_of.as_deref(), telemetry.layer.as_str()), (Some("claude-code"), "Telemetry"));
}

#[test]
fn telemetry_is_otlp_and_creates_no_prompts() {
    let spec = bundled_spec("claude-code-otel");
    assert_eq!((spec.format, spec.prompt_events), (Format::Otlp, Vec::<String>::new()));
    assert_eq!(spec.map["prompt_id"], strings(&["prompt.id"]));
}

#[test]
fn copilot_joins_events_by_session_and_the_others_by_prompt_id() {
    let joins: Vec<Join> = ["claude-code", "codex", "copilot-cli", "cursor"].into_iter().map(|id| bundled_spec(id).join).collect();
    assert_eq!(joins, [Join::PromptId, Join::PromptId, Join::Session, Join::PromptId]);
}

#[test]
fn bundled_spec_inherits_default_map() {
    let spec = bundled_spec("claude-code");
    assert_eq!(spec.map["prompt"][0], "prompt"); // from _defaults.yaml
    assert_eq!(spec.map["session_title"], strings(&["session_title"])); // from the spec
    assert_eq!(spec.usage.events, strings(&["api_request"])); // from _defaults.yaml
}

#[test]
fn claude_code_origin_rules_are_its_own_then_the_shared_ones() {
    let origins: Vec<Origin> = bundled_spec("claude-code").origin.iter().map(|r| r.origin).collect();
    let expected = [
        Origin::Programmatic,
        Origin::TaskNotification,
        Origin::AgentMessage,
        Origin::SlashCommand,
        Origin::SlashCommand,
        Origin::Subagent,
        Origin::Subagent,
        Origin::SlashCommand,
    ];
    assert_eq!(origins, expected);
}

#[test]
fn the_shared_slash_command_rule_matches_from_the_start_of_the_text() {
    let spec = bundled_spec("claude-code");
    let regex = spec.origin.last().unwrap().when.text_regex.clone().unwrap();
    assert!(regex.matches_start("/review the diff") && regex.matches_start("/clear"));
    assert!(!regex.matches_start("please /review"));
}

#[test]
fn claude_code_injected_tags_follow_the_shared_one() {
    let expected = [
        "system-reminder",
        "ide_selection",
        "ide_opened_file",
        "ide_diagnostics",
        "local-command-stdout",
        "local-command-stderr",
        "local-command-caveat",
    ];
    assert_eq!(bundled_spec("claude-code").injected, strings(&expected));
}

#[test]
fn codex_has_its_own_usage_and_capability_since() {
    let spec = bundled_spec("codex");
    assert_eq!((spec.usage.events.clone(), spec.usage.input_includes_cached), (strings(&["PostModelRequest"]), true));
    assert_eq!(spec.usage.values[&UsageValue::InputTokens], "token_usage.input_tokens");
    assert_eq!(spec.capability_since, [(Capability::Usage, "0.164.0".to_string())].into_iter().collect::<IndexMap<_, _>>());
}

#[test]
fn codex_apply_patch_names_the_files_it_touches() {
    let spec = bundled_spec("codex");
    let patch =
        "*** Begin Patch\n*** Add File: a.py\n+x\n*** Update File: src/b.py\n*** Move to: src/c.py\n*** Delete File: d.py\n*** End Patch";
    assert_eq!(spec.tool_files["apply_patch"].first_groups(patch), strings(&["a.py", "src/b.py", "src/c.py", "d.py"]));
}

#[test]
fn codex_yaml_anchors_share_one_hook_between_events() {
    let merge = bundled_spec("codex").install.merge;
    let hooks = &merge["hooks"];
    assert_eq!(hooks["Stop"], hooks["SessionEnd"]);
    assert_eq!(hooks["UserPromptSubmit"], hooks["PostToolUse"]);
    assert_ne!(hooks["Stop"], hooks["UserPromptSubmit"]);
}

#[test]
fn cursor_install_says_what_the_hook_prints_back() {
    let reply = bundled_spec("cursor").install.reply;
    assert_eq!(reply, table(&[("beforeSubmitPrompt", json!({"continue": true})), ("*", json!({}))]));
}

#[test]
fn an_install_without_reply_prints_nothing() {
    assert_eq!((parse(MINIMAL).unwrap().install.reply, bundled_spec("codex").install.reply), (IndexMap::new(), IndexMap::new()));
}

#[test]
fn cursor_answer_replaces_the_shared_one() {
    assert_eq!(bundled_spec("cursor").answer, table(&[("AgentResponse", "text".to_string())]));
    assert_eq!(bundled_spec("claude-code").answer.keys().collect::<Vec<_>>(), ["Stop", "StopFailure"]);
}

#[test]
fn ignored_keys_add_a_spec_own_to_the_shared_ones() {
    let ignored = bundled_spec("codex").ignored;
    let keys: Vec<&str> = ignored.keys().map(String::as_str).collect();
    assert_eq!(keys, ["tool_response", "stop_hook_active", "token_usage.total_tokens", "agent_transcript_path"]);
}

#[test]
fn every_bundled_spec_knows_the_shared_surfaces() {
    let surfaces: Vec<String> = bundled_spec("codex").surfaces.keys().cloned().collect();
    assert_eq!(surfaces, strings(&["cli", "vscode", "jetbrains", "desktop", "cursor", "sdk", "slack", "web"]));
}

#[test]
fn bundled_install_files_expand_under_the_user_home() {
    let settings = settings_with(&[]);
    let files: Vec<PathBuf> =
        ["claude-code", "codex", "cursor"].into_iter().map(|id| expand(&bundled_spec(id).install.file, &settings)).collect();
    let expected = ["/home/ann/.claude/settings.json", "/home/ann/.codex/hooks.json", "/home/ann/.cursor/hooks.json"].map(PathBuf::from);
    assert_eq!(files, expected);
}
