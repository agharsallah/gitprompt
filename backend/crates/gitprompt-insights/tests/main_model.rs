//! The model a prompt shows (`facts.model`, the prompt index `model` column): the model that
//! answered you, i.e. the model of the turn's main-thread calls. Background calls (session titles,
//! web search and fetch helpers, summaries, compaction) and subagents often run another model and
//! can outnumber the main thread's calls; they must never decide the model shown.
//!
//! Nothing here depends on model names: the rule must keep working for models and versions that
//! don't exist yet (the property tests generate arbitrary names).

mod common;

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use serde_json::json;

use common::{event, prompt, spec, utc};
use gitprompt_insights::derivations::{WorkKind, derive, work_kind};
use gitprompt_insights::indexer::entry;
use gitprompt_store::EventRow;

const OPUS: &str = "claude-opus-5-5";
const HAIKU: &str = "claude-haiku-5-5";
const SONNET: &str = "claude-sonnet-5-5";

/// One Claude Code telemetry `api_request`.
fn call(model: &str, query_source: &str) -> EventRow {
    event("api_request", json!({"model": model, "query_source": query_source, "cost_usd": 0.01, "input_tokens": 10}))
}

fn calls(n: usize, model: &str, query_source: &str) -> Vec<EventRow> {
    (0..n).map(|_| call(model, query_source)).collect()
}

/// The model the prompt index stores (and every page shows) for a Claude Code turn.
fn shown(events: &[EventRow]) -> Option<String> {
    shown_with(prompt("do it").build(), "claude-code", events)
}

fn shown_with(view: gitprompt_store::PromptView, source: &str, events: &[EventRow]) -> Option<String> {
    let derived = derive(&view, events, Some(&spec(source)));
    let indexed = entry(&derived, &utc()).expect("entry");
    indexed.row.model
}

// --- regressions, shaped like turns captured live ----------------------------------------------

#[test]
fn web_search_and_fetch_helpers_do_not_replace_the_model_that_answered() {
    // Captured live: Opus answered (12 main-thread calls) while web search/fetch ran on Haiku (19
    // calls); the prompt showed Haiku.
    let events = [
        calls(1, HAIKU, "generate_session_title"),
        calls(1, OPUS, "hook_prompt"),
        calls(4, OPUS, "repl_main_thread"),
        calls(11, HAIKU, "web_search_tool"),
        calls(8, OPUS, "repl_main_thread"),
        calls(8, HAIKU, "web_fetch_apply"),
    ]
    .concat();
    assert_eq!(shown(&events).as_deref(), Some(OPUS));
}

#[test]
fn subagents_on_another_model_do_not_replace_the_model_that_answered() {
    // Captured live: Opus main thread (16 calls), an Explore subagent on Sonnet (31), summaries on
    // Haiku (11); the prompt showed Sonnet.
    let events = [
        calls(1, HAIKU, "generate_session_title"),
        calls(16, OPUS, "repl_main_thread"),
        calls(31, SONNET, "agent:builtin:Explore"),
        calls(11, HAIKU, "agent_summary"),
    ]
    .concat();
    assert_eq!(shown(&events).as_deref(), Some(OPUS));
}

#[test]
fn a_turn_whose_only_call_is_a_background_one_shows_no_model_rather_than_a_wrong_one() {
    // Captured live: only the session-title call (Haiku) had arrived; the prompt showed Haiku.
    assert_eq!(shown(&calls(1, HAIKU, "generate_session_title")), None);
    assert_eq!(shown(&[calls(1, HAIKU, "compact"), calls(3, SONNET, "agent:custom")].concat()), None);
}

#[test]
fn without_main_thread_calls_the_model_the_hooks_reported_is_shown() {
    let view = prompt("do it").field("model", json!(OPUS)).build();
    assert_eq!(shown_with(view, "claude-code", &calls(2, HAIKU, "generate_session_title")).as_deref(), Some(OPUS));
}

#[test]
fn main_thread_calls_win_over_the_model_the_hooks_reported() {
    let view = prompt("do it").field("model", json!(SONNET)).build();
    assert_eq!(shown_with(view, "claude-code", &calls(1, OPUS, "repl_main_thread")).as_deref(), Some(OPUS));
}

#[test]
fn a_programmatic_run_shows_its_own_model() {
    // `claude -p` / the Agent SDK: query_source `sdk`.
    assert_eq!(shown(&[calls(1, HAIKU, "sdk"), calls(1, SONNET, "generate_session_title")].concat()).as_deref(), Some(HAIKU));
}

#[test]
fn a_model_switch_inside_a_turn_shows_the_model_used_most_by_the_main_thread() {
    let events = [calls(1, SONNET, "repl_main_thread"), calls(3, OPUS, "repl_main_thread")].concat();
    assert_eq!(shown(&events).as_deref(), Some(OPUS));
}

#[test]
fn a_tie_between_main_thread_models_goes_to_the_one_called_last_not_to_a_name() {
    // Alphabetical order would pick Sonnet; the user ended the turn on Opus.
    let events = [calls(2, SONNET, "repl_main_thread"), calls(2, OPUS, "repl_main_thread")].concat();
    assert_eq!(shown(&events).as_deref(), Some(OPUS));
    let events = [calls(2, OPUS, "repl_main_thread"), calls(2, SONNET, "repl_main_thread")].concat();
    assert_eq!(shown(&events).as_deref(), Some(SONNET));
}

#[test]
fn main_thread_calls_without_a_model_are_ignored() {
    let events = [vec![event("api_request", json!({"query_source": "repl_main_thread"}))], calls(1, OPUS, "repl_main_thread")].concat();
    assert_eq!(shown(&events).as_deref(), Some(OPUS));
    let empty = event("api_request", json!({"model": "", "query_source": "repl_main_thread"}));
    assert_eq!(shown(&[empty.clone(), empty]), None);
}

#[test]
fn a_tool_that_does_not_say_who_a_call_worked_for_counts_every_call_as_main() {
    // Codex PostModelRequest carries no query_source: every call is the main thread's.
    let codex =
        |model: &str| event("PostModelRequest", json!({"hook_event_name": "PostModelRequest", "model": model, "status": "completed"}));
    let view = prompt("go").source("codex").field("model", json!("gpt-6-luna")).build();
    assert_eq!(shown_with(view, "codex", &[codex("gpt-6-luna"), codex("gpt-6-luna"), codex("gpt-6-mini")]).as_deref(), Some("gpt-6-luna"));
}

#[test]
fn the_turn_still_lists_and_counts_every_model_it_used() {
    let events = [calls(2, OPUS, "repl_main_thread"), calls(5, HAIKU, "web_search_tool")].concat();
    let derived = derive(&prompt("do it").build(), &events, Some(&spec("claude-code")));
    let mut models: Vec<(Option<String>, i64)> =
        entry(&derived, &utc()).expect("entry").models.into_iter().map(|m| (m.model, m.usage.api_requests)).collect();
    models.sort();
    assert_eq!(models, vec![(Some(HAIKU.into()), 5), (Some(OPUS.into()), 2)]);
}

// --- who a call worked for ----------------------------------------------------------------------

#[test]
fn every_query_source_seen_live_is_classified_deliberately() {
    // Every `query_source` Claude Code sent on this project's captures. A new one is system work
    // until it is added here: it then can't change the model shown, at worst it hides it.
    let seen = [
        (None, WorkKind::Main),
        (Some("repl_main_thread"), WorkKind::Main),
        (Some("sdk"), WorkKind::Main),
        (Some("agent:builtin:general-purpose"), WorkKind::Subagent),
        (Some("agent:builtin:Explore"), WorkKind::Subagent),
        (Some("agent:builtin:fork"), WorkKind::Subagent),
        (Some("agent:builtin:claude-code-guide"), WorkKind::Subagent),
        (Some("agent:custom"), WorkKind::Subagent),
        (Some("agent_summary"), WorkKind::System),
        (Some("away_summary"), WorkKind::System),
        (Some("compact"), WorkKind::System),
        (Some("generate_session_title"), WorkKind::System),
        (Some("hook_prompt"), WorkKind::System),
        (Some("prompt_suggestion"), WorkKind::System),
        (Some("web_fetch_apply"), WorkKind::System),
        (Some("web_search_tool"), WorkKind::System),
    ];
    let classified: Vec<_> = seen.iter().map(|(source, _)| (*source, work_kind(*source))).collect();
    assert_eq!(classified, seen.to_vec());
}

#[test]
fn a_main_thread_source_with_a_qualifier_is_still_the_main_thread() {
    // e.g. a custom output style: `repl_main_thread:outputStyle:<name>`.
    assert_eq!(work_kind(Some("repl_main_thread:outputStyle:Explanatory")), WorkKind::Main);
    assert_eq!(
        shown(&[calls(1, OPUS, "repl_main_thread:outputStyle:Learning"), calls(4, HAIKU, "web_search_tool")].concat()).as_deref(),
        Some(OPUS)
    );
}

// --- properties: any model names, any number of background calls, any order ---------------------

/// Model ids, including ones that don't exist yet: any family, version, date, provider prefix.
fn model_id() -> impl Strategy<Value = String> {
    prop_oneof![
        "claude-[a-z]{3,8}-[0-9]{1,2}(-[0-9]{1,2})?(-20[0-9]{6})?(\\[1m\\])?",
        "gpt-[0-9]{1,2}(\\.[0-9])?(-[a-z]{3,6})?",
        "(us|eu)\\.anthropic\\.claude-[a-z]{3,8}-[0-9]-[0-9]-v[0-9]:0",
        "[a-z][a-z0-9.:_/-]{0,30}",
    ]
}

fn background_source() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("generate_session_title".to_string()),
        Just("web_search_tool".to_string()),
        Just("web_fetch_apply".to_string()),
        Just("agent_summary".to_string()),
        Just("compact".to_string()),
        "agent:[a-z:-]{1,20}",
        // Sources Claude Code may add later.
        "[a-z_]{3,20}".prop_filter("not a main-thread source", |s| s != "sdk" && !s.starts_with("repl_main_thread")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn the_main_thread_model_is_shown_whatever_the_names_and_the_background_volume(
        main in model_id(),
        main_calls in 1usize..5,
        background in prop::collection::vec((model_id(), background_source()), 0..40),
        seed in any::<u64>(),
    ) {
        let mut events: Vec<EventRow> = background.iter().map(|(model, source)| call(model, source)).collect();
        // Main-thread calls spread anywhere among the background ones.
        for i in 0..main_calls {
            let at = (seed.wrapping_mul(i as u64 + 1) as usize) % (events.len() + 1);
            events.insert(at, call(&main, "repl_main_thread"));
        }
        prop_assert_eq!(shown(&events), Some(main));
    }

    #[test]
    fn background_calls_alone_never_name_the_model(
        background in prop::collection::vec((model_id(), background_source()), 1..20),
    ) {
        let events: Vec<EventRow> = background.iter().map(|(model, source)| call(model, source)).collect();
        prop_assert_eq!(shown(&events), None);
    }
}
