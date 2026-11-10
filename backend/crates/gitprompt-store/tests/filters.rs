//! Prompt filters (shared by the prompt list, statistics, stories and search): every field, empty
//! values ignored, conditions combined with AND. Port of storage/store.py FILTERS/where/_words and
//! the filter assertions of tests/api/test_stats.py and tests/insights/test_indexer.py.

mod common;

use std::collections::HashMap;

use common::*;
use gitprompt_core::spec::{Origin, ToolKind};
use gitprompt_store::rows::Annotation;
use gitprompt_store::{Has, Outcome, PromptFilter, Store};
use pretty_assertions::assert_eq;

fn texts(store: &Store, filter: &PromptFilter) -> Vec<String> {
    let mut texts: Vec<String> = store.search(filter, 100).unwrap().into_iter().filter_map(|p| p.row.prompt).collect();
    texts.sort();
    texts
}

// --- q: words of the typed text -----------------------------------------------------------------

/// The four prompts of test_indexer.py's search test; the last one's typed text leaves out the
/// injected selection.
fn searchable() -> Store {
    let store = store();
    prompt("p0").text("Fix the Login bug").insert(&store);
    prompt("p1").text("go home").insert(&store);
    prompt("p2").text("say \"quoted\" things").insert(&store);
    prompt("p3").text("<ide_selection>selection</ide_selection> Hi").typed("Hi").insert(&store);
    store
}

fn q(text: &str) -> PromptFilter {
    PromptFilter { q: Some(text.into()), ..Default::default() }
}

macro_rules! q_finds {
    ($($name:ident: $q:expr => [$($text:expr),*];)*) => {$(
        #[test]
        fn $name() {
            let expected: Vec<String> = vec![$($text.to_string()),*];
            assert_eq!(texts(&searchable(), &q($q)), expected);
        }
    )*};
}

q_finds! {
    q_matches_a_word_in_any_case: "login" => ["Fix the Login bug"];
    q_needs_every_word_in_any_order: "LOGIN fix" => ["Fix the Login bug"];
    q_matches_inside_a_word: "ogi" => ["Fix the Login bug"];
    q_matches_words_shorter_than_a_trigram: "go" => ["go home"];
    q_matches_a_one_character_word: "x" => ["Fix the Login bug"];
    q_with_a_short_and_a_long_word_needs_both: "bug go" => [];
    q_treats_a_quote_as_plain_text: "\"quoted" => ["say \"quoted\" things"];
    q_treats_fts_operators_as_plain_words: "login AND" => [];
    q_treats_fts_syntax_characters_as_plain_text: "login*" => [];
    q_ignores_injected_context: "selection" => [];
    q_finds_nothing_for_an_unknown_word: "nothing-like-this" => [];
}

#[test]
fn q_of_only_spaces_keeps_every_prompt() {
    assert_eq!(texts(&searchable(), &q("   ")).len(), 4);
}

#[test]
fn a_prompt_without_typed_text_is_never_found_by_q() {
    let store = store();
    prompt("p1").with(|row| row.typed_text = None).insert(&store);

    assert_eq!(found(&store, &q("hello")), Vec::<i64>::new());
    assert_eq!(found(&store, &q("he")), Vec::<i64>::new());
}

// --- empty values -------------------------------------------------------------------------------

#[test]
fn empty_values_are_ignored() {
    let store = store();
    let ids = vec![prompt("p1").insert(&store), prompt("p2").origin("subagent").insert(&store)];
    let empty = PromptFilter {
        q: Some(String::new()),
        source: Some(String::new()),
        repo: Some(String::new()),
        origin: vec![],
        session: vec![],
        ..Default::default()
    };

    assert_eq!(found(&store, &empty), ids);
}

#[test]
fn starred_false_is_no_filter() {
    // Ported as is: Python treats False as an empty value.
    let store = store();
    let plain = prompt("p1").insert(&store);
    let starred = prompt("p2").insert(&store);
    store.annotate(starred, &Annotation { starred: true, ..Default::default() }).unwrap();

    assert_eq!(found(&store, &PromptFilter { starred: Some(false), ..Default::default() }), vec![plain, starred]);
}

// --- one value per field ------------------------------------------------------------------------

#[test]
fn origin_keeps_any_of_the_listed_origins() {
    let store = store();
    let typed = prompt("p1").origin("typed").insert(&store);
    let slash = prompt("p2").origin("slash_command").insert(&store);
    prompt("p3").origin("task_notification").insert(&store);

    let filter = PromptFilter { origin: vec![Origin::Typed, Origin::SlashCommand], ..Default::default() };
    assert_eq!(found(&store, &filter), vec![typed, slash]);
}

#[test]
fn surface_keeps_any_of_the_listed_surfaces() {
    let store = store();
    let cli = prompt("p1").with(|r| r.surface = Some("cli".into())).insert(&store);
    let ide = prompt("p2").with(|r| r.surface = Some("vscode".into())).insert(&store);
    prompt("p3").with(|r| r.surface = Some("desktop".into())).insert(&store);
    prompt("p4").insert(&store);

    let filter = PromptFilter { surface: vec!["cli".into(), "vscode".into()], ..Default::default() };
    assert_eq!(found(&store, &filter), vec![cli, ide]);
}

#[test]
fn model_keeps_prompts_that_used_any_listed_model() {
    let store = store();
    let both = prompt("p1").model("opus", usage(1, 1.0, 0)).model("haiku", usage(1, 0.1, 0)).insert(&store);
    let haiku = prompt("p2").model("haiku", usage(1, 0.1, 0)).insert(&store);
    prompt("p3").model("sonnet", usage(1, 0.1, 0)).insert(&store);

    let filter = PromptFilter { model: vec!["haiku".into()], ..Default::default() };
    assert_eq!(found(&store, &filter), vec![both, haiku]);
}

#[test]
fn source_keeps_one_source() {
    let store = store();
    prompt("p1").source("claude-code").insert(&store);
    let codex = prompt("p2").source("codex").insert(&store);

    assert_eq!(found(&store, &PromptFilter { source: Some("codex".into()), ..Default::default() }), vec![codex]);
}

#[test]
fn session_keeps_one_or_several_sessions() {
    let store = store();
    let s1 = prompt("p1").session("s1").insert(&store);
    let s2 = prompt("p2").session("s2").insert(&store);
    prompt("p3").session("s3").insert(&store);

    let one = PromptFilter { session: vec!["s2".into()], ..Default::default() };
    let several = PromptFilter { session: vec!["s1".into(), "s2".into()], ..Default::default() };
    assert_eq!(found(&store, &one), vec![s2]);
    assert_eq!(found(&store, &several), vec![s1, s2]);
}

#[test]
fn repo_keeps_one_repository() {
    let store = store();
    let docs = prompt("p1").with(|r| r.repo = Some("/r/docs".into())).insert(&store);
    prompt("p2").with(|r| r.repo = Some("/r/app".into())).insert(&store);

    assert_eq!(found(&store, &PromptFilter { repo: Some("/r/docs".into()), ..Default::default() }), vec![docs]);
}

#[test]
fn since_is_inclusive_and_until_is_exclusive() {
    let store = store();
    prompt("p1").at("2026-11-01T23:59:59.999999+00:00").insert(&store);
    let on_since = prompt("p2").at("2026-11-02T00:00:00.000000+00:00").insert(&store);
    let inside = prompt("p3").at("2026-11-02T12:00:00.000000+00:00").insert(&store);
    prompt("p4").at("2026-11-03T00:00:00.000000+00:00").insert(&store);

    let window = PromptFilter {
        since: Some("2026-11-02T00:00:00.000000+00:00".into()),
        until: Some("2026-11-03T00:00:00.000000+00:00".into()),
        ..Default::default()
    };
    assert_eq!(found(&store, &window), vec![on_since, inside]);
}

#[test]
fn weekday_keeps_one_local_iso_weekday() {
    let store = store();
    let monday = prompt("p1").local("2026-11-02", 9, 1).insert(&store);
    prompt("p2").local("2026-11-03", 9, 2).insert(&store);

    assert_eq!(found(&store, &PromptFilter { weekday: Some(1), ..Default::default() }), vec![monday]);
}

#[test]
fn hour_zero_is_a_value_not_an_empty_filter() {
    let store = store();
    let midnight = prompt("p1").local("2026-11-03", 0, 2).insert(&store);
    prompt("p2").local("2026-11-03", 14, 2).insert(&store);

    assert_eq!(found(&store, &PromptFilter { hour: Some(0), ..Default::default() }), vec![midnight]);
}

#[test]
fn command_keeps_one_slash_command() {
    let store = store();
    let review = prompt("p1").with(|r| r.slash_command = Some("/review".into())).insert(&store);
    prompt("p2").with(|r| r.slash_command = Some("/model".into())).insert(&store);
    prompt("p3").insert(&store);

    assert_eq!(found(&store, &PromptFilter { command: Some("/review".into()), ..Default::default() }), vec![review]);
}

#[test]
fn tool_keeps_prompts_whose_turn_called_that_tool() {
    let store = store();
    let bash = prompt("p1").tool("Read", ToolKind::Read, 2).tool("Bash", ToolKind::Shell, 1).insert(&store);
    prompt("p2").tool("Read", ToolKind::Read, 1).insert(&store);

    assert_eq!(found(&store, &PromptFilter { tool: Some("Bash".into()), ..Default::default() }), vec![bash]);
}

#[test]
fn tool_kind_keeps_prompts_whose_turn_called_a_tool_of_that_kind() {
    let store = store();
    let writes = prompt("p1").tool("Edit", ToolKind::Write, 1).insert(&store);
    prompt("p2").tool("Read", ToolKind::Read, 1).insert(&store);

    assert_eq!(found(&store, &PromptFilter { tool_kind: Some(ToolKind::Write), ..Default::default() }), vec![writes]);
}

#[test]
fn query_source_keeps_prompts_with_model_calls_from_that_source() {
    let store = store();
    let compacted = prompt("p1").work("system", Some("compact"), usage(1, 0.1, 0)).insert(&store);
    prompt("p2").work("main", Some("repl_main_thread"), usage(1, 0.1, 0)).insert(&store);

    let filter = PromptFilter { query_source: Some("compact".into()), ..Default::default() };
    assert_eq!(found(&store, &filter), vec![compacted]);
}

#[test]
fn size_keeps_one_typed_character_bucket() {
    let store = store();
    prompt("p1").typed(&"a".repeat(19)).insert(&store);
    let on_bound = prompt("p2").typed(&"a".repeat(20)).insert(&store);
    let below_next = prompt("p3").typed(&"a".repeat(79)).insert(&store);
    prompt("p4").typed(&"a".repeat(80)).insert(&store);

    assert_eq!(found(&store, &PromptFilter { size: Some("00020".into()), ..Default::default() }), vec![on_bound, below_next]);
}

#[test]
fn the_largest_size_bucket_has_no_upper_bound() {
    let store = store();
    prompt("p1").typed(&"a".repeat(1499)).insert(&store);
    let huge = prompt("p2").typed(&"a".repeat(50_000)).insert(&store);

    assert_eq!(found(&store, &PromptFilter { size: Some("01500".into()), ..Default::default() }), vec![huge]);
}

#[test]
fn before_keeps_prompts_with_a_smaller_id() {
    let store = store();
    let ids: Vec<i64> = (0..3).map(|i| prompt(&format!("p{i}")).insert(&store)).collect();

    assert_eq!(found(&store, &PromptFilter { before: Some(ids[2]), ..Default::default() }), ids[..2].to_vec());
}

#[test]
fn tag_and_starred_follow_the_annotations() {
    let store = store();
    let release = prompt("p1").insert(&store);
    let starred = prompt("p2").insert(&store);
    prompt("p3").insert(&store);
    store.annotate(release, &Annotation { tags: vec!["release".into(), "x".into()], ..Default::default() }).unwrap();
    store.annotate(starred, &Annotation { starred: true, ..Default::default() }).unwrap();

    assert_eq!(found(&store, &PromptFilter { tag: Some("release".into()), ..Default::default() }), vec![release]);
    assert_eq!(found(&store, &PromptFilter { tag: Some("other".into()), ..Default::default() }), Vec::<i64>::new());
    assert_eq!(found(&store, &PromptFilter { starred: Some(true), ..Default::default() }), vec![starred]);
}

#[test]
fn several_filters_must_all_hold() {
    let store = store();
    let both = prompt("p1").origin("typed").with(|r| r.repo = Some("/r/app".into())).insert(&store);
    prompt("p2").origin("typed").with(|r| r.repo = Some("/r/docs".into())).insert(&store);
    prompt("p3").origin("subagent").with(|r| r.repo = Some("/r/app".into())).insert(&store);

    let filter = PromptFilter { origin: vec![Origin::Typed], repo: Some("/r/app".into()), ..Default::default() };
    assert_eq!(found(&store, &filter), vec![both]);
    assert_eq!(store.count(&filter).unwrap(), 1);
}

// --- has ----------------------------------------------------------------------------------------

macro_rules! has_case {
    ($($name:ident: $has:expr => $set:expr;)*) => {$(
        #[test]
        fn $name() {
            let store = store();
            let with = prompt("p1").with($set).insert(&store);
            prompt("p2").session("other").insert(&store);
            let filter = PromptFilter { has: vec![$has], ..Default::default() };
            assert_eq!(found(&store, &filter), vec![with]);
        }
    )*};
}

has_case! {
    has_code_keeps_prompts_with_code_blocks: Has::Code => |r| r.code_blocks = 1;
    has_url_keeps_prompts_with_urls: Has::Url => |r| r.urls = 2;
    has_mention_keeps_prompts_with_mentions: Has::Mention => |r| r.mentions = 1;
    has_secret_keeps_prompts_with_secrets: Has::Secret => |r| r.secrets = 1;
    has_commit_keeps_turns_that_committed: Has::Commit => |r| r.git_commits = 1;
    has_subagent_keeps_turns_that_started_subagents: Has::Subagent => |r| r.subagents = 1;
    has_tool_failure_keeps_turns_with_failed_tool_calls: Has::ToolFailure => |r| r.tool_failures = 1;
    has_api_error_keeps_turns_with_api_errors: Has::ApiError => |r| r.api_errors = 1;
    has_denial_keeps_turns_with_permission_denials: Has::Denial => |r| r.denials = 1;
    has_denial_keeps_turns_with_rejections: Has::Denial => |r| r.rejections = 1;
    has_compaction_keeps_turns_that_compacted: Has::Compaction => |r| r.compactions = 1;
}

#[test]
fn has_no_follow_up_keeps_the_last_prompt_of_each_session() {
    let store = store();
    prompt("p1").session("s1").insert(&store);
    let last = prompt("p2").session("s1").insert(&store);
    let alone = prompt("p3").session("s2").insert(&store);

    assert_eq!(found(&store, &PromptFilter { has: vec![Has::NoFollowUp], ..Default::default() }), vec![last, alone]);
}

#[test]
fn several_has_conditions_must_all_hold() {
    let store = store();
    let both = prompt("p1").with(|r| (r.code_blocks, r.urls) = (1, 1)).insert(&store);
    prompt("p2").with(|r| r.code_blocks = 1).insert(&store);
    prompt("p3").with(|r| r.urls = 1).insert(&store);

    assert_eq!(found(&store, &PromptFilter { has: vec![Has::Code, Has::Url], ..Default::default() }), vec![both]);
}

// --- outcome ------------------------------------------------------------------------------------

const NOW: &str = "2026-11-03T12:00:00.000000+00:00";

/// One prompt per outcome, each in its own session unless noted; "now" is pinned at NOW.
struct Outcomes {
    store: Store,
    done: i64,
    other_end: i64,
    failed: i64,
    interrupted: i64,
    superseded: i64,
    running: i64,
    open: i64,
}

fn outcomes() -> Outcomes {
    let store = store_at(NOW);
    let old = "2026-11-03T09:00:00.000000+00:00";
    let p = |pid: &str, session: &str| prompt(pid).session(session).at(old);
    let done = p("done", "a").ended("Stop", 5.0).insert(&store);
    let other_end = p("other-end", "b").ended("SessionEnd", 5.0).insert(&store);
    let failed = p("failed", "c").ended("StopFailure", 5.0).insert(&store);
    let interrupted = p("interrupted", "d").ended("Interrupt", 5.0).insert(&store);
    let superseded = p("superseded", "e").insert(&store);
    p("follow-up", "e").ended("Stop", 1.0).insert(&store);
    let running = prompt("running").session("f").at("2026-11-03T11:30:00.000000+00:00").insert(&store);
    let open = p("open", "g").insert(&store);
    Outcomes { store, done, other_end, failed, interrupted, superseded, running, open }
}

#[test]
fn outcomes_name_how_each_turn_ended() {
    let o = outcomes();
    let ids = [o.done, o.other_end, o.failed, o.interrupted, o.superseded, o.running, o.open];

    let got = o.store.outcomes(&ids).unwrap();

    let expected = HashMap::from([
        (o.done, Outcome::Done),
        (o.other_end, Outcome::Done),
        (o.failed, Outcome::Failed),
        (o.interrupted, Outcome::Interrupted),
        (o.superseded, Outcome::Superseded),
        (o.running, Outcome::Running),
        (o.open, Outcome::Open),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn outcomes_of_unindexed_prompts_are_left_out() {
    let store = store();
    let unindexed = prompt("p1").insert_unindexed(&store);
    assert_eq!(store.outcomes(&[unindexed]).unwrap(), HashMap::new());
}

#[test]
fn outcome_filter_keeps_any_of_the_listed_outcomes() {
    let o = outcomes();

    let filter = PromptFilter { outcome: vec![Outcome::Failed, Outcome::Interrupted], ..Default::default() };
    assert_eq!(found(&o.store, &filter), vec![o.failed, o.interrupted]);
    assert_eq!(found(&o.store, &PromptFilter { outcome: vec![Outcome::Open], ..Default::default() }), vec![o.open]);
    assert_eq!(found(&o.store, &PromptFilter { outcome: vec![Outcome::Running], ..Default::default() }), vec![o.running]);
}

fn outcome_at(received_at: &str) -> Outcome {
    let store = store_at(NOW);
    let id = prompt("p1").at(received_at).insert(&store);
    store.outcomes(&[id]).unwrap()[&id]
}

#[test]
fn a_turn_sent_exactly_one_hour_ago_is_still_running() {
    assert_eq!(outcome_at("2026-11-03T11:00:00.000000+00:00"), Outcome::Running);
}

#[test]
fn a_turn_sent_just_over_one_hour_ago_without_an_end_is_open() {
    assert_eq!(outcome_at("2026-11-03T10:59:59.000000+00:00"), Outcome::Open);
}

#[test]
fn a_later_prompt_in_the_session_supersedes_even_a_recent_turn() {
    let store = store_at(NOW);
    let recent = prompt("p1").at("2026-11-03T11:50:00.000000+00:00").insert(&store);
    prompt("p2").at("2026-11-03T11:55:00.000000+00:00").insert(&store);

    assert_eq!(store.outcomes(&[recent]).unwrap()[&recent], Outcome::Superseded);
}

#[test]
fn prompts_without_a_session_are_never_superseded() {
    // Ported as is: the follow-up check compares sessions, and no session matches no other.
    let store = store_at(NOW);
    let first = prompt("p1").no_session().at("2026-11-03T09:00:00.000000+00:00").insert(&store);
    prompt("p2").no_session().at("2026-11-03T09:05:00.000000+00:00").insert(&store);

    assert_eq!(store.outcomes(&[first]).unwrap()[&first], Outcome::Open);
}
