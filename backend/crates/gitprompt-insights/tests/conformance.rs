//! Connector conformance, insights side: every prompt fixture under testdata/fixtures gets the origin
//! and typed text its `expect` names, and the facts each connector's prompts show follow its spec's
//! `capabilities` (tokens and cost null, never zeros, when the tool can't report them). Port of the
//! origin and display-contract parts of tests/sources/test_conformance.py; storing and joining are
//! the capture crate's tests.

mod common;

use pretty_assertions::assert_eq;
use serde_json::Value;

use common::{cases, fixture_tools, replay, spec, specs};
use gitprompt_core::spec::{Capability, Spec};
use gitprompt_insights::derivations::{Derived, derive};
use gitprompt_insights::facts::{Connector, Facts, connector};
use gitprompt_insights::indexer::prompt_facts;
use gitprompt_insights::origin::classify;

/// (fixture, what was derived, what the fixture expects) for every prompt fixture of a tool.
fn origins(tool: &str) -> Vec<(String, Value, Value)> {
    let spec = spec(tool);
    let fixtures = replay(&spec, tool);
    let expected: Vec<(String, Value)> = cases(tool).into_iter().map(|c| (c.name, c.expect)).collect();
    fixtures
        .prompts
        .iter()
        .map(|(name, view)| {
            let found = classify(&view.row, &fixtures.turn(name), Some(&spec));
            let expect = &expected.iter().find(|(n, _)| n == name).expect("fixture").1;
            let typed = expect.get("typed_text").map(|_| serde_json::json!(found.typed_text));
            (name.clone(), serde_json::json!([found.origin, typed]), serde_json::json!([expect["origin"], expect.get("typed_text")]))
        })
        .collect()
}

/// Fixtures whose derived origin (or typed text, when the fixture names one) differs from `expect`.
fn mismatches(tool: &str) -> Vec<(String, Value, Value)> {
    origins(tool).into_iter().filter(|(_, found, expected)| found != expected).collect()
}

/// The facts (and derived values) of each prompt fixture of a tool, with the events of its turn.
fn shown(tool: &str) -> Vec<(Facts, Derived)> {
    let spec = spec(tool);
    let fixtures = replay(&spec, tool);
    fixtures
        .prompts
        .iter()
        .map(|(name, view)| {
            let derived = derive(view, &fixtures.turn(name), Some(&spec));
            (prompt_facts(&derived, Some(&spec), None), derived)
        })
        .collect()
}

fn can(spec: &Spec, capability: Capability) -> bool {
    spec.capabilities.contains(&capability)
}

#[test]
fn every_main_bundled_connector_has_fixtures() {
    let tools = fixture_tools();
    let without: Vec<String> = specs().values().filter(|s| s.part_of.is_none() && !tools.contains(&s.id)).map(|s| s.id.clone()).collect();
    assert_eq!(without, Vec::<String>::new());
}

#[test]
fn claude_code_prompts_get_their_expected_origin_and_typed_text() {
    assert_eq!(mismatches("claude-code"), vec![]);
}

#[test]
fn codex_prompts_get_their_expected_origin_and_typed_text() {
    assert_eq!(mismatches("codex"), vec![]);
}

#[test]
fn copilot_prompts_get_their_expected_origin_and_typed_text() {
    assert_eq!(mismatches("copilot-cli"), vec![]);
}

#[test]
fn cursor_prompts_get_their_expected_origin_and_typed_text() {
    assert_eq!(mismatches("cursor"), vec![]);
}

#[test]
fn every_prompt_fixture_of_every_connector_is_checked() {
    let found: Vec<(String, usize)> = fixture_tools()
        .into_iter()
        .map(|t| {
            let n = origins(&t).len();
            (t, n)
        })
        .collect();
    assert_eq!(
        found,
        vec![("claude-code".to_string(), 6), ("codex".to_string(), 3), ("copilot-cli".to_string(), 3), ("cursor".to_string(), 3)]
    );
}

#[test]
fn every_prompt_names_its_connector_from_the_spec() {
    let wrong: Vec<(String, Connector)> = fixture_tools()
        .into_iter()
        .flat_map(|tool| {
            let expected = connector(&tool, Some(&spec(&tool)));
            shown(&tool)
                .into_iter()
                .map(|(f, _)| f.connector)
                .filter(move |c| *c != expected)
                .map(move |c| (tool.clone(), c))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(wrong, vec![]);
}

#[test]
fn codex_prompts_show_their_model_tokens_answer_end_and_tools_but_no_cost() {
    let codex = spec("codex");
    assert!(can(&codex, Capability::Usage) && !can(&codex, Capability::Cost));
    let facts = shown("codex");
    assert!(facts.iter().all(|(f, _)| f.model.is_some()), "a prompt without its model");
    assert!(facts.iter().any(|(f, _)| f.tokens.as_ref().is_some_and(|t| t.total > 0)), "no prompt shows tokens");
    assert!(facts.iter().all(|(f, _)| f.tokens.is_none() || f.api_requests > 0), "tokens without a model call");
    assert!(facts.iter().all(|(f, _)| f.cost_usd.is_none()), "a cost shown though Codex reports none");
    assert!(facts.iter().any(|(f, d)| f.answered && d.summary.response.is_some()), "no prompt shows its answer");
    assert!(facts.iter().any(|(f, d)| f.response_s.is_some() && d.summary.ended_by.is_some()), "no turn end");
    assert!(facts.iter().any(|(f, _)| f.tool_calls > 0), "no tool calls");
}

#[test]
fn claude_code_hook_prompts_show_answer_end_and_tools_but_no_tokens_or_cost() {
    let claude = spec("claude-code");
    assert!(!can(&claude, Capability::Usage) && !can(&claude, Capability::Model));
    let facts = shown("claude-code");
    assert!(facts.iter().all(|(f, _)| f.tokens.is_none() && f.cost_usd.is_none()), "usage shown though the hooks can't report it");
    assert!(facts.iter().any(|(f, d)| f.answered && d.summary.response.is_some()), "no prompt shows its answer");
    assert!(facts.iter().any(|(f, _)| f.response_s.is_some()), "no turn end");
    assert!(facts.iter().any(|(f, _)| f.tool_calls > 0), "no tool calls");
}

#[test]
fn copilot_prompts_show_turn_end_and_tools_but_no_tokens_cost_or_answer() {
    let copilot = spec("copilot-cli");
    assert!(!can(&copilot, Capability::Usage) && !can(&copilot, Capability::Answer));
    let facts = shown("copilot-cli");
    assert!(facts.iter().all(|(f, _)| f.tokens.is_none() && f.cost_usd.is_none()), "usage shown though Copilot can't report it");
    assert!(facts.iter().any(|(f, _)| f.response_s.is_some()), "no turn end");
    assert!(facts.iter().any(|(f, _)| f.tool_calls > 0), "no tool calls");
}

#[test]
fn cursor_prompts_show_model_answer_end_and_tools_but_no_tokens_or_cost() {
    let cursor = spec("cursor");
    assert!(can(&cursor, Capability::Model) && !can(&cursor, Capability::Usage));
    let facts = shown("cursor");
    assert!(facts.iter().all(|(f, _)| f.model.is_some()), "a prompt without its model");
    assert!(facts.iter().all(|(f, _)| f.tokens.is_none() && f.cost_usd.is_none()), "usage shown though Cursor can't report it");
    assert!(facts.iter().any(|(f, d)| f.answered && d.summary.response.is_some()), "no prompt shows its answer");
    assert!(facts.iter().any(|(f, _)| f.response_s.is_some()), "no turn end");
    assert!(facts.iter().any(|(f, _)| f.tool_calls > 0), "no tool calls");
}

#[test]
fn cursor_prompts_report_the_cursor_version_and_no_surface() {
    let facts = shown("cursor");
    assert!(
        facts.iter().all(|(f, _)| f.tool_version.as_deref() == Some("3.4.1") && f.surface.is_none()),
        "{:?}",
        facts.iter().map(|(f, _)| (&f.tool_version, &f.surface)).collect::<Vec<_>>()
    );
}
