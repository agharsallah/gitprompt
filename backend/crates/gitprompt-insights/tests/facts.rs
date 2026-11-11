//! A prompt's facts (facts.rs): one block every page reads, from an index row and the spec. Port of
//! `tests/insights/test_facts.py` and the facts asserted in `tests/api/test_desktop_apps.py`.
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

use pretty_assertions::assert_eq;
use serde_json::json;

use common::{index_row, obj, spec, specs, surface_ref, usage};
use gitprompt_insights::derivations::TurnSummary;
use gitprompt_insights::facts::{Connector, ModelSource, Tokens, attach, connector, facts, main_model};
use gitprompt_store::Outcome;
use gitprompt_store::index::IndexRow;
use gitprompt_store::timeline::IndexedPrompt;

fn summary_with_models(models: &[(Option<&str>, u64)]) -> TurnSummary {
    TurnSummary { models: models.iter().map(|(m, n)| (m.map(String::from), *n)).collect(), ..TurnSummary::default() }
}

fn codex_row(model: Option<&str>) -> IndexRow {
    IndexRow { model: model.map(String::from), ..index_row("codex") }
}

#[test]
fn the_main_model_is_the_main_threads_not_the_one_of_most_model_calls() {
    // The derivation picks the main thread's model (tests/main_model.rs); more calls on another
    // model (background work, subagents) don't change it, nor does the hook-reported model.
    let summary = TurnSummary { main_model: Some("gpt-a".into()), ..summary_with_models(&[(Some("gpt-a"), 1), (Some("gpt-b"), 3)]) };
    assert_eq!(main_model(&summary, &obj(json!({"model": "gpt-c"}))).as_deref(), Some("gpt-a"));
}

#[test]
fn model_calls_that_are_all_background_work_name_no_model() {
    assert_eq!(main_model(&summary_with_models(&[(Some("haiku"), 5)]), &obj(json!({}))), None);
}

#[test]
fn without_model_calls_the_main_model_is_the_one_the_hooks_reported() {
    assert_eq!(main_model(&TurnSummary::default(), &obj(json!({"model": "gpt-6-luna"}))).as_deref(), Some("gpt-6-luna"));
}

#[test]
fn calls_without_a_model_and_an_empty_hook_model_give_no_model() {
    assert_eq!(main_model(&summary_with_models(&[(None, 2)]), &obj(json!({}))), None);
    assert_eq!(main_model(&TurnSummary::default(), &obj(json!({"model": ""}))), None);
}

#[test]
fn no_model_call_means_no_tokens_and_no_cost_not_zeros() {
    let f = facts(&codex_row(Some("gpt-6-luna")), Some(&spec("codex")), Some(Outcome::Done), None);
    assert_eq!((f.tokens, f.cost_usd, f.model_source), (None, None, Some(ModelSource::Hook)));
    assert_eq!(
        f.connector,
        Connector { id: "codex".into(), name: "OpenAI Codex CLI".into(), label: "Codex".into(), assistant: "Codex".into() }
    );
    assert_eq!((f.outcome, f.models), (Some(Outcome::Done), vec!["gpt-6-luna".to_string()]));
}

#[test]
fn tokens_add_up_with_reasoning_counted_once() {
    let row = IndexRow { usage: usage(2, 100, 20, 900, 0.0), reasoning_tokens: 5, ..codex_row(Some("gpt-6-luna")) };
    let f = facts(&row, Some(&spec("codex")), None, None);
    assert_eq!(f.tokens, Some(Tokens { input: 100, output: 20, cache_read: 900, cache_creation: 0, reasoning: 5, total: 1020 }));
    assert_eq!((f.cost_usd, f.model_source, f.api_requests), (None, Some(ModelSource::Usage), 2));
}

#[test]
fn a_free_call_of_a_tool_that_reports_costs_costs_zero() {
    let row = IndexRow { usage: usage(2, 100, 20, 900, 0.0), ..index_row("claude-code") };
    assert_eq!(facts(&row, Some(&spec("claude-code")), None, None).cost_usd, Some(0.0));
}

#[test]
fn a_reported_cost_is_rounded_to_four_decimals() {
    let row = IndexRow { usage: usage(1, 1, 1, 0, 0.123_456), ..index_row("claude-code") };
    assert_eq!(facts(&row, Some(&spec("claude-code")), None, None).cost_usd, Some(0.1235));
}

#[test]
fn a_cost_without_model_calls_is_unknown() {
    let row = IndexRow { usage: usage(0, 0, 0, 0, 0.5), ..index_row("claude-code") };
    assert_eq!(facts(&row, Some(&spec("claude-code")), None, None).cost_usd, None);
}

#[test]
fn calls_without_token_counts_show_no_tokens() {
    let row = IndexRow { usage: usage(1, 0, 0, 0, 0.0), ..codex_row(Some("gpt-6-luna")) };
    let f = facts(&row, Some(&spec("codex")), Some(Outcome::Interrupted), None);
    assert_eq!((f.tokens, f.api_requests, f.model_source), (None, 1, Some(ModelSource::Usage)));
}

#[test]
fn an_unknown_source_is_named_by_its_id() {
    let f = facts(&index_row("mystery"), None, None, None);
    assert_eq!(f.connector, Connector { id: "mystery".into(), name: "mystery".into(), label: "mystery".into(), assistant: String::new() });
    assert_eq!((f.surface, f.cost_usd), (None, None));
}

#[test]
fn a_connector_without_a_short_name_is_labelled_by_its_name() {
    assert_eq!(connector("cursor", Some(&spec("cursor"))).label, "Cursor");
}

#[test]
fn without_a_model_there_is_no_model_source_and_no_models() {
    let f = facts(&index_row("codex"), Some(&spec("codex")), None, None);
    assert_eq!((f.model, f.model_source, f.models), (None, None, Vec::<String>::new()));
}

#[test]
fn every_model_of_the_turn_is_listed_when_given() {
    let f = facts(&codex_row(Some("a")), Some(&spec("codex")), None, Some(vec!["a".into(), "b".into()]));
    assert_eq!(f.models, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn the_surface_is_named_by_the_spec() {
    let row = IndexRow { surface: Some("vscode".into()), ..index_row("claude-code") };
    assert_eq!(facts(&row, Some(&spec("claude-code")), None, None).surface, Some(surface_ref("vscode", "VS Code", "code")));
}

#[test]
fn a_surface_the_spec_does_not_name_is_unknown() {
    let row = IndexRow { surface: Some("hologram".into()), ..index_row("claude-code") };
    assert_eq!(facts(&row, Some(&spec("claude-code")), None, None).surface, None);
}

#[test]
fn what_the_turn_did_and_how_the_tool_reported_it_is_copied() {
    let row = IndexRow {
        permission_mode: Some("plan".into()),
        tool_version: Some("3.4.1".into()),
        response_s: Some(12.5),
        answered: true,
        tool_calls: 4,
        tool_failures: 1,
        files_written: 2,
        git_commits: 1,
        subagents: 3,
        ..index_row("cursor")
    };
    let f = facts(&row, Some(&spec("cursor")), Some(Outcome::Failed), None);
    assert_eq!(
        (f.permission_mode.as_deref(), f.tool_version.as_deref(), f.response_s, f.answered),
        (Some("plan"), Some("3.4.1"), Some(12.5), true)
    );
    assert_eq!(
        (f.tool_calls, f.tool_failures, f.files_written, f.git_commits, f.subagents, f.outcome),
        (4, 1, 2, 1, 3, Some(Outcome::Failed))
    );
}

#[test]
fn facts_serialize_with_the_api_names() {
    let value = serde_json::to_value(facts(&codex_row(Some("gpt-6-luna")), Some(&spec("codex")), Some(Outcome::Superseded), None)).unwrap();
    assert_eq!(
        (value["model_source"].clone(), value["tokens"].clone(), value["outcome"].clone()),
        (json!("hook"), json!(null), json!("superseded"))
    );
}

#[test]
fn attach_gives_each_row_the_facts_of_its_source_and_outcome_in_order() {
    let rows = vec![
        IndexedPrompt { row: IndexRow { id: 1, ..index_row("cursor") }, outcome: Outcome::Open },
        IndexedPrompt { row: IndexRow { id: 2, ..index_row("mystery") }, outcome: Outcome::Done },
    ];
    let attached = attach(rows.clone(), &specs());
    let seen: Vec<(i64, Option<String>, Option<Outcome>)> = attached
        .iter()
        .map(|r| (r.prompt.row.id, r.facts.as_ref().map(|f| f.connector.label.clone()), r.facts.as_ref().and_then(|f| f.outcome)))
        .collect();
    assert_eq!(seen, vec![(1, Some("Cursor".into()), Some(Outcome::Open)), (2, Some("mystery".into()), Some(Outcome::Done))]);
    assert_eq!(attached.into_iter().map(|r| r.prompt).collect::<Vec<_>>(), rows);
}
