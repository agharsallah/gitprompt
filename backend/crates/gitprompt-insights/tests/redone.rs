//! Redone work: failed, interrupted and reworded turns, each counted once, with cost only where
//! the tool reports it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and the figures are small exact values"
)]

use pretty_assertions::assert_eq;

use gitprompt_insights::redone::{Reason, RedonePart, redone};
use gitprompt_store::filter::Outcome;
use gitprompt_store::index::{IndexRow, UsageTotals};
use gitprompt_store::timeline::IndexedPrompt;

/// A typed prompt `id` of session `session` whose turn took `seconds` and ended as `outcome`.
fn turn(id: i64, session: &str, text: &str, seconds: f64, outcome: Outcome) -> IndexedPrompt {
    let row = IndexRow {
        id,
        source: "claude-code".into(),
        session: Some(session.into()),
        origin: "typed".into(),
        typed_text: Some(text.into()),
        response_s: Some(seconds),
        ..IndexRow::default()
    };
    IndexedPrompt { row, outcome }
}

fn costing(mut prompt: IndexedPrompt, usd: f64) -> IndexedPrompt {
    prompt.row.usage = UsageTotals { api_requests: 1, cost_usd: usd, ..UsageTotals::default() };
    prompt
}

const NO_COST: fn(&str) -> bool = |_| false;
const COST: fn(&str) -> bool = |_| true;

#[test]
fn a_failed_turn_you_then_reworded_counts_once_as_failed() {
    let prompts = [
        turn(1, "s", "add a retry loop to the uploader", 30.0, Outcome::Failed),
        turn(2, "s", "add a retry loop to the uploader, with backoff", 40.0, Outcome::Done),
    ];
    let work = redone(&prompts, NO_COST);
    assert_eq!(work.total, RedonePart { turns: 1, agent_time_s: 30.0, cost_usd: None });
    assert_eq!((work.failed.turns, work.reworded.turns), (1, 0));
}

#[test]
fn an_interrupted_turn_you_resumed_counts() {
    let prompts = [
        turn(1, "s", "refactor the store layer", 90.0, Outcome::Interrupted),
        turn(2, "s", "go on, but skip the tests", 60.0, Outcome::Done),
    ];
    let work = redone(&prompts, NO_COST);
    assert_eq!(work.interrupted, RedonePart { turns: 1, agent_time_s: 90.0, cost_usd: None });
    assert_eq!(work.total.turns, 1);
}

#[test]
fn a_prompt_asked_again_in_other_words_is_reworded_and_the_last_version_is_not() {
    let prompts = [
        turn(1, "s", "explain why the parser drops the last line", 20.0, Outcome::Done),
        turn(3, "s", "explain why the parser drops its last line of input", 25.0, Outcome::Done),
        turn(2, "other", "explain why the parser drops the last line", 20.0, Outcome::Done),
    ];
    let work = redone(&prompts, NO_COST);
    assert_eq!(work.reworded.turns, 1, "only the first version in its own session");
    assert_eq!(work.costliest.as_ref().unwrap().id, 1);
    assert_eq!(work.costliest.unwrap().reason, Reason::Reworded);
}

#[test]
fn text_an_agent_sent_is_never_reworded() {
    let mut first = turn(1, "s", "<task-notification>done</task-notification>", 5.0, Outcome::Done);
    first.row.origin = "task_notification".into();
    let mut second = turn(2, "s", "<task-notification>done</task-notification>", 5.0, Outcome::Done);
    second.row.origin = "task_notification".into();
    assert_eq!(redone(&[first, second], NO_COST).total.turns, 0);
}

#[test]
fn a_tool_without_cost_shows_time_only() {
    let prompts = [costing(turn(1, "s", "build it", 30.0, Outcome::Failed), 2.0)];
    let work = redone(&prompts, NO_COST);
    assert_eq!(work.total.cost_usd, None, "the tool reports no cost: no $0");
    assert_eq!(work.total.agent_time_s, 30.0);
}

#[test]
fn cost_is_summed_where_reported_and_names_the_costliest_turn() {
    let prompts = [
        costing(turn(1, "s", "build it", 300.0, Outcome::Failed), 1.5),
        costing(turn(2, "s", "deploy it", 30.0, Outcome::Interrupted), 9.2),
        turn(3, "s", "and now ship it", 10.0, Outcome::Failed),
        costing(turn(4, "s", "say hi", 5.0, Outcome::Done), 0.1),
    ];
    let work = redone(&prompts, COST);
    assert_eq!(work.total, RedonePart { turns: 3, agent_time_s: 340.0, cost_usd: Some(10.7) });
    let costliest = work.costliest.unwrap();
    assert_eq!((costliest.id, costliest.cost_usd, costliest.text.as_deref()), (2, Some(9.2), Some("deploy it")));
}

#[test]
fn nothing_redone_is_an_empty_figure() {
    let work = redone(&[turn(1, "s", "build it", 30.0, Outcome::Done)], NO_COST);
    assert_eq!((work.total.turns, work.costliest), (0, None));
}
