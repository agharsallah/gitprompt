//! Statistics over the prompt index: totals, every grouping, exact splits of multi-valued groupings,
//! split_by, ordering, limit, details/excerpts, quantiles, telemetry, latest. Port of
//! storage/stats.py and tests/api/test_stats.py (seeded with index entries instead of ingest).

mod common;

use std::collections::BTreeMap;

use common::*;
use gitprompt_core::Error;
use gitprompt_core::spec::{Origin, ToolKind};
use gitprompt_store::index::{SessionCommand, SessionMetric};
use gitprompt_store::stats::{Aggregate, AggregateQuery, Bucket, CommandUse, GroupBy, Latest, Order, quantile};
use gitprompt_store::{Outcome, PromptFilter, Store};
use pretty_assertions::assert_eq;

const NOW: &str = "2026-11-04T00:00:00.000000+00:00";

/// Three sessions over two days, two repos, two models, every kind of ending (test_stats.py `data`).
/// Local times are Berlin's: 08:00 UTC on Monday 2 Nov is 09:00.
struct Data {
    store: Store,
}

fn data() -> Data {
    data_at(NOW)
}

fn data_at(now: &str) -> Data {
    let store = store_at(now);
    prompt("p1")
        .text("Fix the login bug")
        .session("s1")
        .at("2026-11-02T08:00:00.000000+00:00")
        .local("2026-11-02", 9, 1)
        .with(|r| {
            r.repo = Some("/r/app".into());
            r.repo_name = Some("me/app".into());
            r.surface = Some("cli".into());
            r.text_hash = Some("h-login".into());
        })
        .ended("Stop", 10.0)
        .tool("Read", ToolKind::Read, 1)
        .tool("Edit", ToolKind::Write, 1)
        .model("opus", usage(1, 1.0, 100))
        .model("haiku", usage(1, 0.1, 100))
        .work("main", None, usage(2, 1.1, 200))
        .insert(&store);
    prompt("p2")
        .text("/review")
        .origin("slash_command")
        .session("s1")
        .at("2026-11-02T08:30:00.000000+00:00")
        .local("2026-11-02", 9, 1)
        .with(|r| {
            r.slash_command = Some("/review".into());
            r.repo = Some("/r/app".into());
            r.repo_name = Some("me/app".into());
            r.surface = Some("cli".into());
            r.text_hash = Some("h-review".into());
        })
        .ended("Stop", 30.0)
        .insert(&store);
    prompt("p3")
        .text("Write the docs")
        .session("s2")
        .at("2026-11-03T13:00:00.000000+00:00")
        .local("2026-11-03", 14, 2)
        .with(|r| {
            r.repo = Some("/r/docs".into());
            r.repo_name = Some("me/docs".into());
            r.surface = Some("cli".into());
            r.text_hash = Some("h-docs".into());
            r.title = Some("Docs work".into());
        })
        .ended("StopFailure", 100.0)
        .tool("Bash", ToolKind::Shell, 1)
        .model("opus", usage(1, 2.0, 100))
        .work("main", None, usage(1, 2.0, 100))
        .insert(&store);
    prompt("p4")
        .text("<task-notification>done</task-notification>")
        .origin("task_notification")
        .session("s3")
        .at("2026-11-03T13:05:00.000000+00:00")
        .local("2026-11-03", 14, 2)
        .with(|r| {
            r.repo = Some("/r/app".into());
            r.repo_name = Some("me/app".into());
            r.text_hash = Some("h-task".into());
        })
        .insert(&store);
    Data { store }
}

fn grouped(group_by: GroupBy) -> AggregateQuery {
    AggregateQuery { group_by: Some(group_by), ..Default::default() }
}

fn filtered(filter: PromptFilter) -> AggregateQuery {
    AggregateQuery { filter, ..Default::default() }
}

fn agg(store: &Store, query: &AggregateQuery) -> Aggregate {
    store.aggregate(query).expect("aggregate")
}

fn totals(store: &Store, filter: PromptFilter) -> Bucket {
    agg(store, &filtered(filter)).totals
}

/// (key, prompts) of each group, in the order returned.
fn keys(groups: &[Bucket]) -> Vec<(Option<String>, i64)> {
    groups.iter().map(|g| (g.key.clone(), g.prompts)).collect()
}

fn measure(groups: &[Bucket], f: impl Fn(&Bucket) -> f64) -> BTreeMap<Option<String>, f64> {
    groups.iter().map(|g| (g.key.clone(), f(g))).collect()
}

fn group<'a>(groups: &'a [Bucket], key: &str) -> &'a Bucket {
    groups.iter().find(|g| g.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no group {key}"))
}

// --- totals -------------------------------------------------------------------------------------

#[test]
fn totals_cover_every_prompt_and_there_are_no_groups_without_group_by() {
    let d = data();

    let result = agg(&d.store, &AggregateQuery::default());

    let t = &result.totals;
    assert_eq!((t.key.clone(), t.prompts, t.sessions, t.ended), (None, 4, 3, 3));
    assert_close(t.cost_usd, 3.1);
    assert_eq!((t.api_requests, t.input_tokens, t.tool_calls), (3.0, 300.0, 3.0));
    assert_eq!((t.response_s_p50, t.response_s_p90), (Some(30.0), Some(86.0)));
    assert_eq!(result.groups, vec![]);
}

#[test]
fn totals_follow_the_filters() {
    let d = data();
    let yours = totals(&d.store, PromptFilter { origin: vec![Origin::Typed, Origin::SlashCommand], ..Default::default() });

    assert_eq!((yours.prompts, yours.sessions), (3, 2));
    assert_eq!(totals(&d.store, PromptFilter { q: Some("docs".into()), ..Default::default() }).prompts, 1);
    let since = PromptFilter { since: Some("2026-11-03T00:00:00+00:00".into()), ..Default::default() };
    assert_eq!(totals(&d.store, since).prompts, 2);
    assert_eq!(totals(&d.store, PromptFilter { model: vec!["haiku".into()], ..Default::default() }).prompts, 1);
    assert_close(totals(&d.store, PromptFilter { repo: Some("/r/docs".into()), ..Default::default() }).cost_usd, 2.0);
}

#[test]
fn totals_of_no_prompts_are_zero_without_quantiles() {
    let t = totals(&store(), PromptFilter::default());
    assert_eq!(t, Bucket::default());
}

#[test]
fn every_measure_is_summed_over_the_kept_prompts() {
    let store = store();
    prompt("a")
        .text("see https://x.y and @README.md")
        .at("2026-11-03T10:00:00.000000+00:00")
        .with(|r| {
            (r.code_blocks, r.urls, r.mentions, r.secrets) = (1, 1, 1, 1);
            (r.git_commits, r.subagent_tool_calls, r.subagents, r.files_written) = (1, 2, 1, 3);
            (r.compactions, r.denials, r.rejections, r.api_errors, r.tool_failures) = (1, 1, 2, 1, 1);
            (r.api_time_s, r.tool_time_s) = (0.44, 1.04);
            (r.usage.output_tokens, r.usage.cache_read_tokens, r.usage.cache_creation_tokens) = (5, 6, 7);
            r.text_hash = Some("same".into());
        })
        .ended("StopFailure", 2.0)
        .insert(&store);
    prompt("b")
        .at("2026-11-03T11:00:00.000000+00:00")
        .with(|r| {
            r.tool_time_s = 0.02;
            r.text_hash = Some("same".into());
        })
        .insert(&store);

    let t = totals(&store, PromptFilter::default());

    let counts = [t.with_code, t.with_url, t.with_mention, t.with_secret, t.with_commit, t.git_commits];
    assert_eq!(counts, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!([t.subagent_tool_calls, t.subagents, t.files_written, t.compactions], [2.0, 1.0, 3.0, 1.0]);
    assert_eq!([t.denials, t.rejections, t.api_errors, t.tool_failures, t.failed], [1.0, 2.0, 1.0, 1.0, 1.0]);
    assert_eq!([t.output_tokens, t.cache_read_tokens, t.cache_creation_tokens], [5.0, 6.0, 7.0]);
    assert_eq!((t.api_time_s, t.tool_time_s), (0.4, 1.1), "rounded to 1 decimal");
    assert_eq!((t.followed, t.ended, t.distinct_texts), (1.0, 1, 1));
    assert_eq!(
        (t.first_at.as_deref(), t.last_at.as_deref()),
        (Some("2026-11-03T10:00:00.000000+00:00"), Some("2026-11-03T11:00:00.000000+00:00"))
    );
}

// --- every grouping -----------------------------------------------------------------------------

macro_rules! group_case {
    ($($name:ident: $group_by:expr => [$(($key:expr, $prompts:expr)),*];)*) => {$(
        #[test]
        fn $name() {
            let expected: Vec<(Option<String>, i64)> = vec![$(($key.map(String::from), $prompts)),*];
            assert_eq!(keys(&agg(&data().store, &grouped($group_by)).groups), expected);
        }
    )*};
}

const NULL: Option<&str> = None;

group_case! {
    day_groups_by_local_calendar_day_in_key_order: GroupBy::Day => [(Some("2026-11-02"), 2), (Some("2026-11-03"), 2)];
    day_hour_groups_by_local_clock_hour_in_key_order: GroupBy::DayHour => [(Some("2026-11-02 09"), 2), (Some("2026-11-03 14"), 2)];
    hour_groups_by_zero_padded_local_hour_in_key_order: GroupBy::Hour => [(Some("09"), 2), (Some("14"), 2)];
    weekday_groups_by_iso_weekday_in_key_order: GroupBy::Weekday => [(Some("1"), 2), (Some("2"), 2)];
    weekday_hour_groups_by_weekday_and_hour_in_key_order: GroupBy::WeekdayHour => [(Some("1-09"), 2), (Some("2-14"), 2)];
    origin_groups_most_prompts_first_then_by_key: GroupBy::Origin => [(Some("typed"), 2), (Some("slash_command"), 1), (Some("task_notification"), 1)];
    source_groups_by_connector: GroupBy::Source => [(Some("claude-code"), 4)];
    surface_groups_unknown_surfaces_under_null: GroupBy::Surface => [(Some("cli"), 3), (NULL, 1)];
    session_groups_by_session: GroupBy::Session => [(Some("s1"), 2), (Some("s2"), 1), (Some("s3"), 1)];
    session_length_buckets_sessions_by_first_prompt_to_last_turn_end: GroupBy::SessionLength => [(Some("00000"), 1), (Some("00060"), 1), (Some("01800"), 2)];
    repo_groups_by_repository: GroupBy::Repo => [(Some("/r/app"), 3), (Some("/r/docs"), 1)];
    branch_groups_prompts_without_a_branch_under_null: GroupBy::Branch => [(NULL, 4)];
    command_groups_by_slash_command: GroupBy::Command => [(NULL, 3), (Some("/review"), 1)];
    size_buckets_typed_characters_in_key_order: GroupBy::Size => [(Some("00000"), 3), (Some("00020"), 1)];
    text_groups_by_text_hash: GroupBy::Text => [(Some("h-docs"), 1), (Some("h-login"), 1), (Some("h-review"), 1), (Some("h-task"), 1)];
    outcome_groups_by_how_turns_ended: GroupBy::Outcome => [(Some("done"), 2), (Some("failed"), 1), (Some("open"), 1)];
    model_counts_a_prompt_in_every_model_it_used: GroupBy::Model => [(Some("opus"), 2), (Some("haiku"), 1)];
    tool_counts_a_prompt_in_every_tool_it_called: GroupBy::Tool => [(Some("Bash"), 1), (Some("Edit"), 1), (Some("Read"), 1)];
    tool_kind_counts_a_prompt_in_every_kind_it_called: GroupBy::ToolKind => [(Some("read"), 1), (Some("shell"), 1), (Some("write"), 1)];
    work_groups_by_kind_of_work: GroupBy::Work => [(Some("main"), 2)];
    query_source_groups_model_calls_without_a_source_under_null: GroupBy::QuerySource => [(NULL, 2)];
}

#[test]
fn session_length_counts_every_prompt_of_a_session_whatever_the_filters() {
    let d = data();
    let typed =
        AggregateQuery { filter: PromptFilter { origin: vec![Origin::Typed], ..Default::default() }, ..grouped(GroupBy::SessionLength) };

    // s1 keeps only p1 (10 s on its own) but its span is still 08:00 to 08:30:30.
    let expected = vec![(Some("00060".to_string()), 1), (Some("01800".to_string()), 1)];
    assert_eq!(keys(&agg(&d.store, &typed).groups), expected);
}

#[test]
fn prompts_without_a_session_have_no_session_length() {
    let store = store();
    prompt("p1").no_session().insert(&store);
    assert_eq!(agg(&store, &grouped(GroupBy::SessionLength)).groups, vec![]);
}

#[test]
fn outcome_groups_include_superseded_and_running_turns() {
    let d = data_at("2026-11-03T13:30:00.000000+00:00");
    prompt("p7").session("s3").at("2026-11-03T13:20:00.000000+00:00").insert(&d.store);

    let groups = agg(&d.store, &grouped(GroupBy::Outcome)).groups;

    let found: BTreeMap<Option<String>, i64> = keys(&groups).into_iter().collect();
    let expected = BTreeMap::from([
        (Some("done".to_string()), 2),
        (Some("failed".to_string()), 1),
        (Some("running".to_string()), 1),
        (Some("superseded".to_string()), 1),
    ]);
    assert_eq!(found, expected);
}

#[test]
fn an_interrupted_turn_has_its_own_outcome_group_and_filter() {
    let store = store();
    prompt("p1").ended("Interrupt", 4.0).insert(&store);

    assert_eq!(keys(&agg(&store, &grouped(GroupBy::Outcome)).groups), vec![(Some("interrupted".to_string()), 1)]);
    let filter = PromptFilter { outcome: vec![Outcome::Interrupted], ..Default::default() };
    assert_eq!(totals(&store, filter).prompts, 1);
}

// --- exact splits of multi-valued groupings ----------------------------------------------------

#[test]
fn usage_is_split_exactly_between_models() {
    let d = data();
    let result = agg(&d.store, &grouped(GroupBy::Model));

    let cost = measure(&result.groups, |g| g.cost_usd);
    assert_close(cost[&Some("opus".to_string())], 3.0);
    assert_close(cost[&Some("haiku".to_string())], 0.1);
    let requests = measure(&result.groups, |g| g.api_requests);
    assert_eq!(requests, BTreeMap::from([(Some("haiku".to_string()), 1.0), (Some("opus".to_string()), 2.0)]));
    let tokens = measure(&result.groups, |g| g.input_tokens);
    assert_eq!(tokens, BTreeMap::from([(Some("haiku".to_string()), 100.0), (Some("opus".to_string()), 200.0)]));
    assert_close(cost.values().sum(), result.totals.cost_usd);
}

#[test]
fn tool_calls_are_split_exactly_between_tool_kinds_and_tools() {
    let store = store();
    prompt("p1").tool("Read", ToolKind::Read, 3).tool("Grep", ToolKind::Read, 2).tool("Bash", ToolKind::Shell, 1).insert(&store);

    let kinds = measure(&agg(&store, &grouped(GroupBy::ToolKind)).groups, |g| g.tool_calls);
    let tools = measure(&agg(&store, &grouped(GroupBy::Tool)).groups, |g| g.tool_calls);

    assert_eq!(kinds, BTreeMap::from([(Some("read".to_string()), 5.0), (Some("shell".to_string()), 1.0)]));
    assert_eq!(tools[&Some("Read".to_string())], 3.0);
}

/// One prompt with model calls of four query sources (two of them system work), one main-only.
fn work_data() -> Store {
    let store = store();
    prompt("p1")
        .model("opus", usage(4, 1.0, 0))
        .work("main", Some("repl_main_thread"), usage(1, 0.5, 0))
        .work("subagent", Some("agent:builtin:Explore"), usage(1, 0.25, 0))
        .work("system", Some("compact"), usage(1, 0.125, 0))
        .work("system", Some("away_summary"), usage(1, 0.125, 0))
        .insert(&store);
    prompt("p2").model("opus", usage(1, 1.0, 0)).work("main", Some("repl_main_thread"), usage(1, 1.0, 0)).insert(&store);
    store
}

#[test]
fn usage_is_split_by_kind_of_work_and_adds_up_to_the_totals() {
    let store = work_data();
    let result = agg(&store, &grouped(GroupBy::Work));

    let cost = measure(&result.groups, |g| g.cost_usd);
    assert_eq!(
        cost,
        BTreeMap::from([(Some("main".to_string()), 1.5), (Some("subagent".to_string()), 0.25), (Some("system".to_string()), 0.25)])
    );
    assert_close(cost.values().sum(), result.totals.cost_usd);
    assert_eq!(group(&result.groups, "system").prompts, 1, "two system sources, one prompt");
    assert_eq!(group(&result.groups, "system").api_requests, 2.0);
}

#[test]
fn query_source_groups_name_their_kind_of_work() {
    let store = work_data();
    let groups = agg(&store, &grouped(GroupBy::QuerySource)).groups;

    assert_eq!(group(&groups, "agent:builtin:Explore").detail.as_deref(), Some("subagent"));
    assert_eq!(group(&groups, "repl_main_thread").prompts, 2);
    assert_close(group(&groups, "repl_main_thread").cost_usd, 1.5);
    let compact = PromptFilter { query_source: Some("compact".into()), ..Default::default() };
    assert_eq!(totals(&store, compact).prompts, 1);
}

// --- details and excerpts -----------------------------------------------------------------------

#[test]
fn a_session_group_is_named_by_its_title_else_the_start_of_its_first_prompt_and_has_a_span() {
    let d = data();
    let groups = agg(&d.store, &grouped(GroupBy::Session)).groups;

    let s1 = group(&groups, "s1");
    assert_eq!((s1.detail.as_deref(), s1.span_s), (Some("Fix the login bug"), Some(1830.0)));
    assert_eq!(group(&groups, "s2").detail.as_deref(), Some("Docs work"));
    assert_eq!(group(&groups, "s3").span_s, Some(0.0));
}

#[test]
fn only_session_groups_have_a_span() {
    let groups = agg(&data().store, &grouped(GroupBy::Repo)).groups;
    assert_eq!(groups.iter().map(|g| g.span_s).collect::<Vec<_>>(), vec![None, None]);
}

#[test]
fn a_repo_group_is_named_by_its_remote() {
    let groups = agg(&data().store, &grouped(GroupBy::Repo)).groups;
    assert_eq!(group(&groups, "/r/docs").detail.as_deref(), Some("me/docs"));
}

#[test]
fn a_tool_group_names_the_tools_kind() {
    let groups = agg(&data().store, &grouped(GroupBy::Tool)).groups;
    assert_eq!(group(&groups, "Bash").detail.as_deref(), Some("shell"));
}

#[test]
fn a_text_group_shows_the_start_of_its_last_occurrence_and_repeats_lower_distinct_texts() {
    let store = store();
    prompt("p1").typed("fix the login bug").with(|r| r.text_hash = Some("h".into())).insert(&store);
    prompt("p2").typed("fix the LOGIN   bug").with(|r| r.text_hash = Some("h".into())).insert(&store);
    prompt("p3").typed("other").with(|r| r.text_hash = Some("o".into())).insert(&store);

    let result = agg(&store, &grouped(GroupBy::Text));

    let top = &result.groups[0];
    assert_eq!((top.prompts, top.detail.as_deref()), (2, Some("fix the LOGIN   bug")));
    assert_eq!(result.totals.prompts - result.totals.distinct_texts, 1);
}

#[test]
fn an_excerpt_is_the_first_200_characters() {
    let store = store();
    let long = format!("{}{}", "é".repeat(200), "x".repeat(100));
    prompt("p1").typed(&long).with(|r| r.text_hash = Some("h".into())).insert(&store);

    let groups = agg(&store, &grouped(GroupBy::Text)).groups;

    assert_eq!(groups[0].detail, Some("é".repeat(200)));
}

// --- quantiles ----------------------------------------------------------------------------------

#[test]
fn each_group_has_response_time_quantiles_of_its_ended_turns() {
    let d = data();
    let days = agg(&d.store, &grouped(GroupBy::Day)).groups;
    let outcomes = agg(&d.store, &grouped(GroupBy::Outcome)).groups;

    assert_eq!((days[0].response_s_p50, days[1].response_s_p90), (Some(20.0), Some(100.0)));
    let open = group(&outcomes, "open");
    assert_eq!((open.ended, open.response_s_p50, open.response_s_p90), (0, None, None));
}

#[test]
fn a_quantile_of_no_values_is_none() {
    assert_eq!(quantile(&[], 0.5), None);
}

#[test]
fn a_quantile_interpolates_linearly_between_neighbours() {
    assert_eq!(quantile(&[10.0, 30.0, 100.0], 0.5), Some(30.0));
    assert_eq!(quantile(&[10.0, 30.0, 100.0], 0.9), Some(86.0));
    assert_eq!(quantile(&[1.0, 2.0], 0.5), Some(1.5));
}

#[test]
fn a_quantile_is_rounded_to_one_decimal() {
    assert_eq!(quantile(&[0.0, 1.0], 0.33), Some(0.3));
    assert_eq!(quantile(&[0.0, 1.0], 0.36), Some(0.4));
}

#[test]
fn the_extreme_quantiles_are_the_smallest_and_largest_values() {
    let values = [1.0, 4.0, 9.0];
    assert_eq!((quantile(&values, 0.0), quantile(&values, 1.0)), (Some(1.0), Some(9.0)));
    assert_eq!(quantile(&[7.0], 0.9), Some(7.0));
}

// --- order and limit ----------------------------------------------------------------------------

#[test]
fn limit_caps_the_number_of_groups() {
    let query = AggregateQuery { limit: 1, ..grouped(GroupBy::Session) };
    assert_eq!(keys(&agg(&data().store, &query).groups), vec![(Some("s1".to_string()), 2)]);
}

#[test]
fn groups_can_be_ordered_by_a_measure_descending() {
    let d = data();
    let ordered = |group_by, order| {
        let query = AggregateQuery { order: Some(order), ..grouped(group_by) };
        agg(&d.store, &query).groups.into_iter().map(|g| g.key.unwrap()).collect::<Vec<_>>()
    };

    assert_eq!(ordered(GroupBy::Repo, Order::CostUsd), ["/r/docs", "/r/app"]);
    assert_eq!(ordered(GroupBy::Session, Order::LastAt), ["s3", "s2", "s1"]);
    assert_eq!(ordered(GroupBy::Repo, Order::Sessions), ["/r/app", "/r/docs"]);
    assert_eq!(ordered(GroupBy::Origin, Order::Key), ["slash_command", "task_notification", "typed"]);
}

#[test]
fn ordering_by_prompts_overrides_the_key_order_of_time_buckets() {
    let store = store();
    prompt("p1").local("2026-11-01", 9, 7).insert(&store);
    prompt("p2").local("2026-11-02", 9, 1).insert(&store);
    prompt("p3").local("2026-11-02", 10, 1).insert(&store);
    let query = AggregateQuery { order: Some(Order::Prompts), ..grouped(GroupBy::Day) };

    let expected = vec![(Some("2026-11-02".to_string()), 2), (Some("2026-11-01".to_string()), 1)];
    assert_eq!(keys(&agg(&store, &query).groups), expected);
}

// --- split_by -----------------------------------------------------------------------------------

fn parts(result: &Aggregate, f: impl Fn(&Bucket) -> f64) -> BTreeMap<Option<String>, BTreeMap<Option<String>, f64>> {
    result.groups.iter().map(|g| (g.key.clone(), measure(g.parts.as_deref().expect("parts"), &f))).collect()
}

fn split(group_by: GroupBy, split_by: GroupBy) -> AggregateQuery {
    AggregateQuery { split_by: Some(split_by), ..grouped(group_by) }
}

fn s(key: &str) -> Option<String> {
    Some(key.to_string())
}

#[test]
fn split_by_gives_each_group_its_parts() {
    let d = data();

    let by_day = parts(&agg(&d.store, &split(GroupBy::Repo, GroupBy::Day)), |p| p.prompts as f64);

    let expected = BTreeMap::from([
        (s("/r/app"), BTreeMap::from([(s("2026-11-02"), 2.0), (s("2026-11-03"), 1.0)])),
        (s("/r/docs"), BTreeMap::from([(s("2026-11-03"), 1.0)])),
    ]);
    assert_eq!(by_day, expected);
}

#[test]
fn parts_of_a_multi_valued_grouping_split_its_measures() {
    let d = data();

    let kinds = parts(&agg(&d.store, &split(GroupBy::Repo, GroupBy::ToolKind)), |p| p.tool_calls);

    let expected = BTreeMap::from([
        (s("/r/app"), BTreeMap::from([(s("read"), 1.0), (s("write"), 1.0)])),
        (s("/r/docs"), BTreeMap::from([(s("shell"), 1.0)])),
    ]);
    assert_eq!(kinds, expected);
}

#[test]
fn parts_add_up_to_their_groups() {
    let result = agg(&data().store, &split(GroupBy::Outcome, GroupBy::Repo));
    let total: i64 = result.groups.iter().flat_map(|g| g.parts.clone().unwrap()).map(|p| p.prompts).sum();
    assert_eq!(total, 4);
}

#[test]
fn a_null_group_gets_its_parts_too() {
    let by_day = parts(&agg(&data().store, &split(GroupBy::Branch, GroupBy::Day)), |p| p.prompts as f64);
    assert_eq!(by_day, BTreeMap::from([(None, BTreeMap::from([(s("2026-11-02"), 2.0), (s("2026-11-03"), 2.0)]))]));
}

#[test]
fn groups_have_no_parts_without_split_by() {
    let groups = agg(&data().store, &grouped(GroupBy::Repo)).groups;
    assert_eq!(groups.iter().map(|g| g.parts.clone()).collect::<Vec<_>>(), vec![None, None]);
}

fn rejected(query: AggregateQuery) -> bool {
    matches!(data().store.aggregate(&query), Err(Error::Invalid(_)))
}

#[test]
fn split_by_two_multi_valued_groupings_is_invalid() {
    assert!(rejected(split(GroupBy::Model, GroupBy::Tool)));
}

#[test]
fn split_by_the_same_grouping_is_invalid() {
    assert!(rejected(split(GroupBy::Repo, GroupBy::Repo)));
}

#[test]
fn split_by_without_group_by_is_invalid() {
    assert!(rejected(AggregateQuery { split_by: Some(GroupBy::Repo), ..Default::default() }));
}

// --- telemetry ----------------------------------------------------------------------------------

const METRIC: &str = "claude_code.lines_of_code.count[added]";

fn metric(event_id: i64, session: &str, received_at: &str, value: f64) -> SessionMetric {
    SessionMetric { event_id, session: Some(session.into()), received_at: received_at.into(), name: METRIC.into(), value }
}

fn command(event_id: i64, session: &str, command: &str) -> SessionCommand {
    SessionCommand {
        event_id,
        session: Some(session.into()),
        received_at: "2026-11-02T08:01:00.000000+00:00".into(),
        command: command.into(),
        source: Some("builtin".into()),
    }
}

fn with_telemetry() -> Data {
    let d = data();
    let metrics = [
        metric(1, "s1", "2026-11-02T08:10:00.000000+00:00", 4.0),
        metric(2, "s1", "2026-11-02T08:40:00.000000+00:00", 6.0),
        metric(3, "s2", "2026-11-03T13:01:00.000000+00:00", 5.0),
        metric(4, "s9", "2026-11-03T13:01:00.000000+00:00", 100.0),
    ];
    let commands = [command(5, "s1", "/model"), command(6, "s1", "/model"), command(7, "s2", "/clear")];
    d.store.index().add_telemetry(7, &metrics, &commands).unwrap();
    d
}

#[test]
fn telemetry_is_summed_over_the_sessions_of_the_kept_prompts() {
    let d = with_telemetry();
    let yours = PromptFilter { origin: vec![Origin::Typed, Origin::SlashCommand], ..Default::default() };

    let result = d.store.telemetry(&yours).unwrap();

    assert_eq!(result.sessions, 2);
    assert_eq!(result.metrics.into_iter().collect::<Vec<_>>(), vec![(METRIC.to_string(), 15.0)]);
    assert_eq!(
        result.commands,
        vec![
            CommandUse { command: "/model".into(), source: Some("builtin".into()), uses: 2, sessions: 1 },
            CommandUse { command: "/clear".into(), source: Some("builtin".into()), uses: 1, sessions: 1 },
        ]
    );
}

#[test]
fn telemetry_follows_the_prompt_filters() {
    let d = with_telemetry();
    let docs = d.store.telemetry(&PromptFilter { repo: Some("/r/docs".into()), ..Default::default() }).unwrap();
    assert_eq!((docs.sessions, docs.metrics.get(METRIC).copied()), (1, Some(5.0)));
}

#[test]
fn telemetry_counts_only_data_points_within_since_and_until() {
    let d = with_telemetry();
    let window = PromptFilter {
        since: Some("2026-11-02T08:15:00.000000+00:00".into()),
        until: Some("2026-11-03T00:00:00.000000+00:00".into()),
        ..Default::default()
    };

    let result = d.store.telemetry(&window).unwrap();

    assert_eq!((result.sessions, result.metrics.get(METRIC).copied()), (1, Some(6.0)));
    assert_eq!(result.commands, vec![]);
}

#[test]
fn telemetry_after_every_prompt_is_empty() {
    let d = with_telemetry();
    let result = d.store.telemetry(&PromptFilter { since: Some("2099-01-01T00:00:00+00:00".into()), ..Default::default() }).unwrap();
    assert_eq!(result, Default::default());
}

#[test]
fn telemetry_metric_totals_are_rounded_to_four_decimals() {
    let store = store();
    prompt("p1").session("s1").insert(&store);
    let metrics = [metric(1, "s1", T0, 1.00004), metric(2, "s1", T0, 1.00002)];
    store.index().add_telemetry(2, &metrics, &[]).unwrap();

    assert_eq!(store.telemetry(&PromptFilter::default()).unwrap().metrics[METRIC], 2.0001);
}

// --- latest -------------------------------------------------------------------------------------

#[test]
fn latest_lists_the_newest_prompt_of_each_connector_and_surface_newest_first() {
    let store = store();
    let surface = |s: &'static str| move |r: &mut gitprompt_store::index::IndexRow| r.surface = Some(s.into());
    prompt("p1").with(surface("cli")).insert(&store);
    let cli = prompt("p2")
        .session("s2")
        .with(|r| {
            r.surface = Some("cli".into());
            r.repo = Some("/r/app".into());
            r.repo_name = Some("me/app".into());
            r.branch = Some("main".into());
            r.title = Some("T".into());
        })
        .model("opus", usage(1, 0.1, 0))
        .insert(&store);
    let unknown = prompt("p3").no_session().origin("subagent").insert(&store);
    let codex = prompt("p4").source("codex").with(surface("cli")).at("2026-11-03T11:00:00.000000+00:00").insert(&store);

    let latest = store.latest().unwrap();

    let row = |id, source: &str, surface: Option<&str>, at: &str, session: Option<&str>, origin: &str, prompts| Latest {
        id,
        source: source.into(),
        surface: surface.map(Into::into),
        received_at: at.into(),
        session: session.map(Into::into),
        repo: None,
        repo_name: None,
        branch: None,
        model: None,
        origin: origin.into(),
        title: None,
        prompts,
    };
    let cli_row = Latest {
        repo: Some("/r/app".into()),
        repo_name: Some("me/app".into()),
        branch: Some("main".into()),
        model: Some("opus".into()),
        title: Some("T".into()),
        ..row(cli, "claude-code", Some("cli"), T0, Some("s2"), "typed", 2)
    };
    assert_eq!(
        latest,
        vec![
            row(codex, "codex", Some("cli"), "2026-11-03T11:00:00.000000+00:00", Some("s1"), "typed", 1),
            row(unknown, "claude-code", None, T0, None, "subagent", 1),
            cli_row,
        ]
    );
}
