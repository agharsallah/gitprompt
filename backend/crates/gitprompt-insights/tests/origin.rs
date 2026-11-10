//! Origin and typed text (origin.rs), from the bundled specs' rules, on synthetic prompts. Port of
//! tests/insights/test_origin.py, plus the rule branches and block boundaries it left implicit.

mod common;

use pretty_assertions::assert_eq;
use serde_json::json;

use common::{custom_spec, event, prompt, spec, spec_yaml, telemetry, with_origin_rules};
use gitprompt_core::spec::{Origin, OriginRule, Pattern, When};
use gitprompt_insights::origin::{Classified, Injected, classify, matches, origin_of, split_context};
use gitprompt_store::EventRow;

const TAGS: [&str; 2] = ["ide_selection", "system-reminder"];

fn tags() -> Vec<String> {
    TAGS.iter().map(|t| t.to_string()).collect()
}

/// The origin Claude Code's spec gives a prompt with this text and these turn events.
fn claude_origin(text: &str, events: &[EventRow]) -> Origin {
    classify(&prompt(text).row(), events, Some(&spec("claude-code"))).origin
}

fn kinds(injected: &[Injected]) -> Vec<&str> {
    injected.iter().map(|i| i.kind.as_str()).collect()
}

// --- classify with Claude Code's rules ------------------------------------------------------------

#[test]
fn a_plain_instruction_is_typed() {
    assert_eq!(claude_origin("Rename the parser module", &[]), Origin::Typed);
}

#[test]
fn a_path_at_the_start_is_typed_not_a_slash_command() {
    assert_eq!(claude_origin("/usr/local is missing from PATH", &[]), Origin::Typed);
}

#[test]
fn a_slash_command_with_arguments_is_a_slash_command() {
    assert_eq!(claude_origin("/review the last commit", &[]), Origin::SlashCommand);
}

#[test]
fn a_bare_slash_command_is_a_slash_command() {
    assert_eq!(claude_origin("/compact", &[]), Origin::SlashCommand);
}

#[test]
fn a_namespaced_slash_command_with_dots_and_dashes_is_a_slash_command() {
    assert_eq!(claude_origin("/plugin:run.v2-beta now", &[]), Origin::SlashCommand);
}

#[test]
fn a_slash_command_followed_by_punctuation_is_typed() {
    assert_eq!(claude_origin("/review, please", &[]), Origin::Typed);
}

#[test]
fn a_slash_command_in_the_middle_of_the_text_is_typed() {
    assert_eq!(claude_origin("please run /review", &[]), Origin::Typed);
}

#[test]
fn an_expanded_command_event_makes_a_slash_command() {
    assert_eq!(claude_origin("deploy staging", &[event("UserPromptExpansion", json!({"command_name": "deploy"}))]), Origin::SlashCommand);
}

#[test]
fn a_telemetry_command_name_makes_a_slash_command() {
    let events = [telemetry(json!({"command_name": "deploy", "app.entrypoint": "cli"}))];
    assert_eq!(claude_origin("deploy staging", &events), Origin::SlashCommand);
}

#[test]
fn a_telemetry_prompt_with_an_empty_command_name_stays_typed() {
    assert_eq!(claude_origin("deploy staging", &[telemetry(json!({"command_name": ""}))]), Origin::Typed);
}

#[test]
fn a_task_notification_is_recognised_by_its_prefix() {
    assert_eq!(claude_origin("<task-notification><task-id>t1</task-id></task-notification>", &[]), Origin::TaskNotification);
}

#[test]
fn an_agent_message_is_recognised_by_its_prefix() {
    assert_eq!(claude_origin(r#"<agent-message from="worker">done</agent-message>"#, &[]), Origin::AgentMessage);
}

#[test]
fn a_prompt_with_an_agent_id_field_is_a_subagent() {
    let row = prompt("Summarise the diff").field("agent_id", json!("a1")).row();
    assert_eq!(classify(&row, &[], Some(&spec("claude-code"))).origin, Origin::Subagent);
}

#[test]
fn a_prompt_with_an_agent_type_field_is_a_subagent() {
    let row = prompt("Summarise the diff").field("agent_type", json!("Explore")).row();
    assert_eq!(classify(&row, &[], Some(&spec("claude-code"))).origin, Origin::Subagent);
}

#[test]
fn an_empty_or_null_field_does_not_satisfy_a_field_rule() {
    let claude = spec("claude-code");
    let empty = prompt("Summarise the diff").field("agent_id", json!("")).row();
    let null = prompt("Summarise the diff").field("agent_id", json!(null)).row();
    let empty_list = prompt("Summarise the diff").field("agent_id", json!([])).row();
    assert_eq!(
        [
            classify(&empty, &[], Some(&claude)).origin,
            classify(&null, &[], Some(&claude)).origin,
            classify(&empty_list, &[], Some(&claude)).origin
        ],
        [Origin::Typed; 3]
    );
}

#[test]
fn a_zero_field_is_a_value_for_a_field_rule() {
    let row = prompt("Summarise the diff").field("agent_id", json!(0)).row();
    assert_eq!(classify(&row, &[], Some(&spec("claude-code"))).origin, Origin::Subagent);
}

#[test]
fn a_prompt_sent_through_the_sdk_is_programmatic() {
    assert_eq!(claude_origin("Grade this answer", &[telemetry(json!({"app.entrypoint": "sdk-cli"}))]), Origin::Programmatic);
}

#[test]
fn a_prompt_typed_in_the_ide_extension_is_typed() {
    assert_eq!(claude_origin("Grade this answer", &[telemetry(json!({"app.entrypoint": "claude-vscode"}))]), Origin::Typed);
}

#[test]
fn only_the_named_event_carries_an_event_rule() {
    let api_request = event("api_request", json!({"app.entrypoint": "sdk-cli"}));
    assert_eq!(claude_origin("Grade this answer", &[api_request]), Origin::Typed);
}

#[test]
fn the_spec_own_rules_win_over_the_shared_ones() {
    let row = prompt("<task-notification>x</task-notification>").field("agent_id", json!("a1")).row();
    assert_eq!(classify(&row, &[], Some(&spec("claude-code"))).origin, Origin::TaskNotification);
}

#[test]
fn only_injected_context_means_nothing_was_typed() {
    let out = classify(&prompt("<ide_selection>x = 1</ide_selection>").row(), &[], Some(&spec("claude-code")));
    assert_eq!(
        out,
        Classified {
            origin: Origin::Unknown,
            typed_text: Some(String::new()),
            injected: vec![Injected { kind: "ide_selection".into(), chars: 36 }]
        }
    );
}

#[test]
fn injected_context_is_removed_before_the_text_rules_run() {
    let text = "<system-reminder>be brief</system-reminder>\n<task-notification>x</task-notification>";
    let out = classify(&prompt(text).row(), &[], Some(&spec("claude-code")));
    assert_eq!((out.origin, out.typed_text.as_deref()), (Origin::TaskNotification, Some("<task-notification>x</task-notification>")));
}

#[test]
fn a_prompt_without_text_is_unknown_with_no_typed_text() {
    let out = classify(&prompt("x").no_text().row(), &[], Some(&spec("claude-code")));
    assert_eq!(out, Classified { origin: Origin::Unknown, typed_text: None, injected: vec![] });
}

#[test]
fn a_whitespace_only_prompt_is_unknown_with_empty_typed_text() {
    let out = classify(&prompt("  \n ").row(), &[], Some(&spec("claude-code")));
    assert_eq!((out.origin, out.typed_text), (Origin::Unknown, Some(String::new())));
}

#[test]
fn without_a_spec_the_origin_is_unknown_and_the_text_is_kept_as_received() {
    let out = classify(&prompt(" <ide_selection>a</ide_selection> hello ").row(), &[], None);
    assert_eq!(
        out,
        Classified { origin: Origin::Unknown, typed_text: Some(" <ide_selection>a</ide_selection> hello ".into()), injected: vec![] }
    );
}

#[test]
fn copilot_marks_prompts_with_a_parent_agent_as_subagents() {
    let row = prompt("look at the logs").source("copilot-cli").field("parent_agent_id", json!("main")).row();
    assert_eq!(classify(&row, &[], Some(&spec("copilot-cli"))).origin, Origin::Subagent);
}

// --- rules from spec data --------------------------------------------------------------------------

fn rule(when: When, origin: Origin) -> OriginRule {
    OriginRule { when, origin }
}

fn path_rule(path: &str, equals: Option<serde_json::Value>, origin: Origin) -> OriginRule {
    rule(When { path: Some(path.into()), equals, ..When::default() }, origin)
}

#[test]
fn a_path_rule_with_equals_matches_only_that_value() {
    let custom = with_origin_rules(&spec("claude-code"), vec![path_rule("mode", Some(json!("batch")), Origin::Programmatic)]);
    let batch = prompt("hi").raw("mode", json!("batch")).row();
    let chat = prompt("hi").raw("mode", json!("chat")).row();
    assert_eq!(
        (classify(&batch, &[], Some(&custom)).origin, classify(&chat, &[], Some(&custom)).origin),
        (Origin::Programmatic, Origin::Typed)
    );
}

#[test]
fn equals_compares_json_values_not_their_text() {
    let custom = with_origin_rules(&spec("claude-code"), vec![path_rule("mode", Some(json!(1)), Origin::Programmatic)]);
    let text_one = prompt("hi").raw("mode", json!("1")).row();
    let number_one = prompt("hi").raw("mode", json!(1)).row();
    assert_eq!(
        (classify(&text_one, &[], Some(&custom)).origin, classify(&number_one, &[], Some(&custom)).origin),
        (Origin::Typed, Origin::Programmatic)
    );
}

#[test]
fn a_path_rule_without_equals_needs_a_value_there() {
    let custom = with_origin_rules(&spec("claude-code"), vec![path_rule("via", None, Origin::AgentMessage)]);
    let via = prompt("hi").raw("via", json!("bus")).row();
    let empty = prompt("hi").raw("via", json!("")).row();
    let absent = prompt("hi").row();
    assert_eq!(
        [
            classify(&via, &[], Some(&custom)).origin,
            classify(&empty, &[], Some(&custom)).origin,
            classify(&absent, &[], Some(&custom)).origin
        ],
        [Origin::AgentMessage, Origin::Typed, Origin::Typed]
    );
}

#[test]
fn a_path_rule_reads_nested_payload_paths() {
    let custom = with_origin_rules(&spec("claude-code"), vec![path_rule("client.kind", Some(json!("bot")), Origin::Programmatic)]);
    let row = prompt("hi").raw("client", json!({"kind": "bot"})).row();
    assert_eq!(classify(&row, &[], Some(&custom)).origin, Origin::Programmatic);
}

#[test]
fn every_condition_of_a_rule_must_hold() {
    let when = When { text_prefix: Some("BUILD:".into()), field: Some("agent_id".into()), ..When::default() };
    let custom = with_origin_rules(&spec("claude-code"), vec![rule(when, Origin::Programmatic)]);
    let both = prompt("BUILD: nightly").field("agent_id", json!("a")).row();
    let prefix_only = prompt("BUILD: nightly").row();
    assert_eq!(
        (classify(&both, &[], Some(&custom)).origin, classify(&prefix_only, &[], Some(&custom)).origin),
        (Origin::Programmatic, Origin::Typed)
    );
}

#[test]
fn an_event_rule_without_a_path_matches_any_event_of_that_name() {
    let when = When { event: Some("Notification".into()), ..When::default() };
    let typed = "hi";
    assert!(matches(&when, &prompt(typed).row(), &[event("Notification", json!({}))], typed));
    assert!(!matches(&when, &prompt(typed).row(), &[event("Stop", json!({}))], typed));
}

#[test]
fn a_text_regex_must_match_at_the_start_of_the_typed_text() {
    let when = When { text_regex: Some(Pattern::new(r"run\b").unwrap()), ..When::default() };
    assert!(matches(&when, &prompt("x").row(), &[], "run it"));
    assert!(!matches(&when, &prompt("x").row(), &[], "please run it"));
}

#[test]
fn text_rules_look_at_the_typed_text_given_not_the_raw_prompt() {
    let when = When { text_prefix: Some("Fix".into()), ..When::default() };
    assert!(matches(&when, &prompt("<ide_selection>a</ide_selection> Fix").row(), &[], "Fix"));
}

#[test]
fn the_first_matching_rule_gives_the_origin_else_typed() {
    let rules = vec![path_rule("a", None, Origin::Programmatic), path_rule("b", None, Origin::AgentMessage)];
    let both = prompt("hi").raw("a", json!(1)).raw("b", json!(1)).row();
    assert_eq!(origin_of(&both, &[], &rules, "hi"), Origin::Programmatic);
    assert_eq!(origin_of(&prompt("hi").row(), &[], &rules, "hi"), Origin::Typed);
}

#[test]
fn a_parsed_spec_rule_reclassifies_with_no_code_change() {
    let custom = custom_spec(&spec_yaml("origin: [{when: {text_prefix: 'BUILD:'}, origin: programmatic}]\n"));
    assert_eq!(classify(&prompt("BUILD: nightly").row(), &[], Some(&custom)).origin, Origin::Programmatic);
}

#[test]
fn spec_rules_come_before_the_shared_ones_and_tags_add_up() {
    let claude = spec("claude-code");
    let origins: Vec<Origin> = claude.origin.iter().map(|r| r.origin).collect();
    assert_eq!((origins.first(), origins.last()), (Some(&Origin::Programmatic), Some(&Origin::SlashCommand)));
    assert!(claude.injected.contains(&"system-reminder".to_string()) && claude.injected.contains(&"ide_selection".to_string()));
}

// --- split_context ---------------------------------------------------------------------------------

#[test]
fn text_without_blocks_is_only_trimmed() {
    assert_eq!(split_context("  Fix the bug\n", &tags()), ("Fix the bug".to_string(), vec![]));
}

#[test]
fn without_tags_nothing_is_removed() {
    assert_eq!(
        split_context(" <system-reminder>x</system-reminder> hi ", &[]),
        ("<system-reminder>x</system-reminder> hi".to_string(), vec![])
    );
}

#[test]
fn a_leading_block_with_attributes_is_removed() {
    let (typed, injected) = split_context("<ide_selection path=\"a.py\">x = 1</ide_selection>\nFix the bug", &tags());
    assert_eq!((typed.as_str(), kinds(&injected)), ("Fix the bug", vec!["ide_selection"]));
}

#[test]
fn a_trailing_block_is_removed() {
    let (typed, injected) = split_context("Fix the bug\n<system-reminder>be brief</system-reminder>", &tags());
    assert_eq!((typed.as_str(), kinds(&injected)), ("Fix the bug", vec!["system-reminder"]));
}

#[test]
fn several_leading_and_trailing_blocks_are_removed_in_text_order() {
    let text = "<ide_selection>a</ide_selection><system-reminder>b</system-reminder> Fix <system-reminder>c</system-reminder>";
    let (typed, injected) = split_context(text, &tags());
    assert_eq!((typed.as_str(), kinds(&injected)), ("Fix", vec!["ide_selection", "system-reminder", "system-reminder"]));
}

#[test]
fn trailing_blocks_are_reported_in_text_order_not_removal_order() {
    let (typed, injected) = split_context("Fix <ide_selection>x</ide_selection><system-reminder>y</system-reminder>", &tags());
    assert_eq!(
        (typed.as_str(), injected),
        ("Fix", vec![Injected { kind: "ide_selection".into(), chars: 32 }, Injected { kind: "system-reminder".into(), chars: 36 }])
    );
}

#[test]
fn a_text_that_is_only_a_block_leaves_nothing_typed() {
    let (typed, injected) = split_context("<ide_selection>only</ide_selection>", &tags());
    assert_eq!((typed.as_str(), kinds(&injected)), ("", vec!["ide_selection"]));
}

#[test]
fn a_block_in_the_middle_is_part_of_the_prompt() {
    let text = "Compare <ide_selection>a</ide_selection> with b";
    assert_eq!(split_context(text, &tags()), (text.to_string(), vec![]));
}

#[test]
fn an_unclosed_block_stays() {
    let text = "<ide_selection>unclosed\nFix the bug";
    assert_eq!(split_context(text, &tags()), (text.to_string(), vec![]));
}

#[test]
fn a_tag_that_is_not_listed_stays() {
    let text = "<pasted_content>x</pasted_content> Fix";
    assert_eq!(split_context(text, &tags()), (text.to_string(), vec![]));
}

#[test]
fn a_block_may_not_contain_another_opening_of_its_tag() {
    let text = "<system-reminder>a<system-reminder>b</system-reminder> hi";
    assert_eq!(split_context(text, &tags()), (text.to_string(), vec![]));
}

#[test]
fn a_tag_does_not_match_a_longer_tag_name() {
    let text = "<ide_selection>x</ide_selection> hi";
    assert_eq!(split_context(text, &["ide".to_string()]), (text.to_string(), vec![]));
}

#[test]
fn tag_names_are_literal_text_not_patterns() {
    let tags = vec!["a.b".to_string()];
    assert_eq!(split_context("<a.b>x</a.b> hi", &tags), ("hi".to_string(), vec![Injected { kind: "a.b".into(), chars: 12 }]));
    assert_eq!(split_context("<aXb>x</aXb> hi", &tags), ("<aXb>x</aXb> hi".to_string(), vec![]));
}

#[test]
fn blocks_spanning_lines_and_surrounding_blank_lines_are_removed() {
    let (typed, injected) = split_context("\n\n<system-reminder>line one\nline two</system-reminder>\n\nFix\n", &tags());
    assert_eq!((typed.as_str(), kinds(&injected)), ("Fix", vec!["system-reminder"]));
}

#[test]
fn a_block_size_counts_its_characters() {
    let (_, injected) = split_context("<system-reminder>abc</system-reminder> hi", &tags());
    assert_eq!(injected, vec![Injected { kind: "system-reminder".into(), chars: "<system-reminder>abc</system-reminder>".len() }]);
}

#[test]
fn a_block_size_counts_characters_not_bytes() {
    let (_, injected) = split_context("<system-reminder>é</system-reminder> hi", &tags());
    assert_eq!(injected, vec![Injected { kind: "system-reminder".into(), chars: 36 }]);
}
