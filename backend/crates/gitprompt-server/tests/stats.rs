//! GET /api/stats and /api/stats/telemetry: the prompt filters plus group_by / split_by / order,
//! computed on the prompt index; and the prompt list taking the same filters. Port of
//! test_stats.py.
//!
//! Time zone: grouping by day/hour/weekday uses the process's local time zone. The fixture's times
//! keep the same calendar day in every zone from UTC-11 to UTC+10; local hours are computed with
//! `jiff::tz::TimeZone::system()`, so the assertions hold wherever the suite runs (the Python suite pinned TZ to
//! Europe/Berlin, which a Rust test can't do without mutating the process environment).

mod common;

use std::collections::BTreeMap;

use common::{Api, api, groups, num};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// (prompt_id, text, session, cwd, received_at)
const PROMPTS: &[(&str, &str, &str, &str, &str)] = &[
    ("p1", "Fix the login bug", "s1", "/r/app", "2026-11-02T11:00:00.000000+00:00"), // a Monday
    ("p2", "/review", "s1", "/r/app", "2026-11-02T11:30:00.000000+00:00"),
    ("p3", "Write the docs", "s2", "/r/docs", "2026-11-03T13:00:00.000000+00:00"), // a Tuesday
    ("p4", "<task-notification>done</task-notification>", "s3", "/r/app", "2026-11-03T13:05:00.000000+00:00"),
];

/// Three sessions over two days, two repos, two models, every kind of ending (synthetic).
async fn data() -> Api {
    let api = api();
    for (pid, text, session, cwd, _) in PROMPTS {
        api.ingest(
            "claude-code",
            json!({"hook_event_name": "UserPromptSubmit", "prompt_id": pid, "session_id": session, "prompt": text, "cwd": cwd}),
        )
        .await;
    }
    let events = [
        ("p1", "PostToolUse", "2026-11-02T11:00:05", json!({"tool_name": "Read", "tool_input": {"file_path": "/a"}})),
        ("p1", "PostToolUse", "2026-11-02T11:00:06", json!({"tool_name": "Edit", "tool_input": {"file_path": "/a"}})),
        ("p1", "Stop", "2026-11-02T11:00:10", json!({})),
        ("p2", "Stop", "2026-11-02T11:30:30", json!({})),
        ("p3", "PostToolUse", "2026-11-03T13:00:05", json!({"tool_name": "Bash", "tool_input": {"command": "make"}})),
        ("p3", "StopFailure", "2026-11-03T13:01:40", json!({})),
        ("p1", "api_request", "2026-11-03T13:00:01", json!({"model": "opus", "cost_usd": 1.0, "input_tokens": 100})),
        ("p1", "api_request", "2026-11-03T13:00:01", json!({"model": "haiku", "cost_usd": 0.1, "input_tokens": 100})),
        ("p3", "api_request", "2026-11-03T13:00:01", json!({"model": "opus", "cost_usd": 2.0, "input_tokens": 100})),
    ];
    let mut statements = String::new();
    for (pid, _, _, _, at) in PROMPTS {
        statements += &format!("update prompt set received_at = '{at}' where json_extract(fields, '$.prompt_id') = '{pid}';");
    }
    for (id, (pid, name, at, extra)) in (1..).zip(events) {
        let payload = common::with(&json!({"hook_event_name": name, "prompt_id": pid, "session_id": "x"}), extra);
        api.ingest("claude-code", payload).await;
        statements += &format!("update event set received_at = '{at}.000000+00:00' where id = {id};");
    }
    api.sql(&statements);
    api
}

async fn stats(api: &Api, query: &[(&str, &str)]) -> Value {
    api.json_with("/api/stats", query).await
}

async fn totals(api: &Api, query: &[(&str, &str)]) -> Value {
    stats(api, query).await["totals"].clone()
}

fn local(at: &str, format: &str) -> String {
    at.parse::<Timestamp>().expect("time").to_zoned(TimeZone::system()).strftime(format).to_string()
}

/// Prompts per local-time key of the fixture prompts (`%H`, `%u`...).
fn local_counts(format: &str) -> BTreeMap<String, f64> {
    let mut counts = BTreeMap::new();
    for (.., at) in PROMPTS {
        *counts.entry(local(at, format)).or_insert(0.0) += 1.0;
    }
    counts
}

fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

// --- totals ---

#[tokio::test]
async fn totals_cover_every_prompt_without_groups() {
    let api = data().await;
    let result = stats(&api, &[]).await;
    let t = &result["totals"];
    assert_eq!(
        (num(&t["prompts"]), num(&t["sessions"]), num(&t["api_requests"]), num(&t["input_tokens"]), num(&t["tool_calls"])),
        (4.0, 3.0, 3.0, 300.0, 3.0)
    );
    assert!((num(&t["cost_usd"]) - 3.1).abs() < 1e-9, "{}", t["cost_usd"]);
    assert_eq!((num(&t["ended"]), num(&t["response_s_p50"]), num(&t["response_s_p90"])), (3.0, 30.0, 86.0));
    assert_eq!(result["groups"], json!([]));
}

#[tokio::test]
async fn totals_follow_the_filters() {
    let api = data().await;
    let yours = totals(&api, &[("origin", "typed"), ("origin", "slash_command")]).await;
    assert_eq!((num(&yours["prompts"]), num(&yours["sessions"])), (3.0, 2.0));
    assert_eq!(num(&totals(&api, &[("q", "docs")]).await["prompts"]), 1.0);
    assert_eq!(num(&totals(&api, &[("since", "2026-11-03T00:00:00+00:00")]).await["prompts"]), 2.0);
    assert_eq!(num(&totals(&api, &[("until", "2026-11-03T00:00:00")]).await["prompts"]), 2.0); // no offset: local time
    assert_eq!(num(&totals(&api, &[("model", "haiku")]).await["prompts"]), 1.0);
    assert_eq!(num(&totals(&api, &[("repo", "/r/docs")]).await["cost_usd"]), 2.0);
}

// --- each grouping ---

async fn check_group(group_by: &str, expected: BTreeMap<String, f64>, by_key: bool) {
    let api = data().await;
    let result = stats(&api, &[("group_by", group_by)]).await;
    assert_eq!(result["group_by"], group_by);
    assert_eq!(groups(&result, "prompts"), expected);
    let keys: Vec<String> = result["groups"].as_array().expect("groups").iter().map(|g| common::key(&g["key"])).collect();
    let counts: Vec<f64> = result["groups"].as_array().expect("groups").iter().map(|g| num(&g["prompts"])).collect();
    let in_order = if by_key { keys.windows(2).all(|w| w[0] <= w[1]) } else { counts.windows(2).all(|w| w[0] >= w[1]) };
    assert!(in_order, "{group_by}: groups not ordered by {}: {keys:?} {counts:?}", if by_key { "key" } else { "prompts" });
}

macro_rules! group_by {
    ($($name:ident: $group:literal => $expected:expr, by_key: $by_key:literal;)*) => {$(
        #[tokio::test]
        async fn $name() {
            check_group($group, $expected, $by_key).await;
        }
    )*};
}

group_by! {
    groups_by_local_day: "day" => map(&[("2026-11-02", 2.0), ("2026-11-03", 2.0)]), by_key: true;
    groups_by_local_day_and_hour: "day_hour" => local_counts("%Y-%m-%d %H"), by_key: true;
    groups_by_local_hour: "hour" => local_counts("%H"), by_key: true;
    groups_by_iso_weekday: "weekday" => map(&[("1", 2.0), ("2", 2.0)]), by_key: true;
    groups_by_weekday_and_hour: "weekday_hour" => local_counts("%u-%H"), by_key: true;
    groups_by_origin: "origin" => map(&[("typed", 2.0), ("slash_command", 1.0), ("task_notification", 1.0)]), by_key: false;
    groups_by_source: "source" => map(&[("claude-code", 4.0)]), by_key: false;
    groups_by_session: "session" => map(&[("s1", 2.0), ("s2", 1.0), ("s3", 1.0)]), by_key: false;
    groups_by_repo: "repo" => map(&[("/r/app", 3.0), ("/r/docs", 1.0)]), by_key: false;
    groups_by_branch_with_none_outside_git: "branch" => map(&[("null", 4.0)]), by_key: false;
    groups_by_session_length_bucket: "session_length" => map(&[("00000", 1.0), ("00060", 1.0), ("01800", 2.0)]), by_key: true;
    groups_by_typed_size_bucket: "size" => map(&[("00000", 3.0), ("00020", 1.0)]), by_key: true;
    groups_by_work: "work" => map(&[("main", 2.0)]), by_key: false;
    groups_by_query_source_with_none_for_the_main_thread: "query_source" => map(&[("null", 2.0)]), by_key: false;
    groups_by_command: "command" => map(&[("null", 3.0), ("/review", 1.0)]), by_key: false;
    groups_by_outcome: "outcome" => map(&[("done", 2.0), ("failed", 1.0), ("open", 1.0)]), by_key: false;
    groups_by_model_counting_a_prompt_in_each: "model" => map(&[("opus", 2.0), ("haiku", 1.0)]), by_key: false;
    groups_by_tool: "tool" => map(&[("Read", 1.0), ("Edit", 1.0), ("Bash", 1.0)]), by_key: false;
    groups_by_tool_kind: "tool_kind" => map(&[("read", 1.0), ("write", 1.0), ("shell", 1.0)]), by_key: false;
}

#[tokio::test]
async fn every_grouping_name_is_accepted() {
    let api = api();
    let names = [
        "day",
        "day_hour",
        "hour",
        "weekday",
        "weekday_hour",
        "origin",
        "source",
        "surface",
        "session",
        "session_length",
        "repo",
        "branch",
        "command",
        "size",
        "text",
        "outcome",
        "model",
        "tool",
        "tool_kind",
        "work",
        "query_source",
    ];
    let mut statuses = Vec::new();
    for name in names {
        statuses.push((name, api.get("/api/stats").query(&[("group_by", name)]).send().await.status));
    }
    assert_eq!(statuses, names.map(|n| (n, 200)));
}

// --- measures, limits, validation ---

#[tokio::test]
async fn usage_is_split_by_model_and_calls_by_tool_kind() {
    let api = data().await;
    let by_model = stats(&api, &[("group_by", "model")]).await;
    assert_eq!(groups(&by_model, "cost_usd"), map(&[("opus", 3.0), ("haiku", 0.1)]));
    assert_eq!(groups(&by_model, "api_requests"), map(&[("opus", 2.0), ("haiku", 1.0)]));
    assert_eq!(
        groups(&stats(&api, &[("group_by", "tool_kind")]).await, "tool_calls"),
        map(&[("read", 1.0), ("write", 1.0), ("shell", 1.0)])
    );
    let by_day = stats(&api, &[("group_by", "day")]).await["groups"].clone();
    assert_eq!((num(&by_day[0]["response_s_p50"]), num(&by_day[1]["response_s_p90"])), (20.0, 100.0));
}

#[tokio::test]
async fn the_limit_caps_the_groups() {
    let api = data().await;
    assert_eq!(stats(&api, &[("group_by", "session"), ("limit", "1")]).await["groups"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn invalid_stats_queries_are_422() {
    let api = api();
    let queries = [
        "group_by=nope",
        "origin=nope",
        "order=nope&group_by=repo",
        "limit=0",
        "limit=10001",
        "weekday=8",
        "hour=24",
        "has=nope",
        "outcome=nope",
        "group_by=model&split_by=tool",
        "group_by=repo&split_by=repo",
        "split_by=repo",
    ];
    let mut statuses = Vec::new();
    for query in queries {
        statuses.push((query, api.get(&format!("/api/stats?{query}")).send().await.status));
    }
    assert_eq!(statuses, queries.map(|q| (q, 422)));
}

#[tokio::test]
async fn the_prompt_list_takes_the_same_filters() {
    let api = data().await;
    let texts = |list: Value| list.as_array().expect("list").iter().map(|p| p["prompt"].clone()).collect::<Vec<_>>();
    assert_eq!(
        texts(api.json_with("/api/prompts", &[("origin", "typed"), ("origin", "slash_command")]).await),
        [json!("Write the docs"), json!("/review"), json!("Fix the login bug")]
    );
    assert_eq!(texts(api.json("/api/prompts?origin=task_notification").await), [json!("<task-notification>done</task-notification>")]);
    assert_eq!(api.json("/api/prompts").await.as_array().map(Vec::len), Some(4)); // no origin: all of them
    assert_eq!(texts(api.json("/api/prompts?q=LOGIN").await), [json!("Fix the login bug")]);
}

async fn check_filter(query: &[(&str, &str)], expected: f64) {
    let api = data().await;
    assert_eq!(num(&totals(&api, query).await["prompts"]), expected, "stats {query:?}");
    assert_eq!(api.json_with("/api/prompts", query).await.as_array().map(Vec::len), Some(expected as usize), "prompts {query:?}");
}

macro_rules! filter {
    ($($name:ident: $query:expr => $expected:expr;)*) => {$(
        #[tokio::test]
        async fn $name() {
            check_filter($query, $expected).await;
        }
    )*};
}

filter! {
    the_weekday_filter_matches_its_group: &[("weekday", "1")] => 2.0;
    the_command_filter_matches_its_group: &[("command", "/review")] => 1.0;
    the_tool_filter_matches_its_group: &[("tool", "Bash")] => 1.0;
    the_tool_kind_filter_matches_its_group: &[("tool_kind", "write")] => 1.0;
    several_outcomes_are_alternatives: &[("outcome", "done"), ("outcome", "failed")] => 3.0;
    one_outcome_filters_alone: &[("outcome", "open")] => 1.0;
    the_size_filter_matches_its_bucket: &[("size", "00020")] => 1.0;
    the_has_filter_keeps_prompts_without_a_follow_up: &[("has", "no_follow_up")] => 3.0;
    several_sessions_are_alternatives: &[("session", "s1"), ("session", "s2")] => 3.0;
    one_session_filters_alone: &[("session", "s2")] => 1.0;
}

#[tokio::test]
async fn the_hour_filter_keeps_prompts_of_that_local_hour() {
    let hour = local("2026-11-03T13:00:00+00:00", "%-H");
    check_filter(&[("hour", &hour)], local_counts("%H")[&local("2026-11-03T13:00:00+00:00", "%H")]).await;
}

#[tokio::test]
async fn hour_zero_is_a_value_not_an_empty_filter() {
    let expected = local_counts("%H").get("00").copied().unwrap_or(0.0);
    check_filter(&[("hour", "0")], expected).await;
}

#[tokio::test]
async fn remap_rebuilds_the_index_with_the_same_prompts() {
    let api = data().await;
    let before = num(&totals(&api, &[]).await["prompts"]);
    assert_eq!(api.post("/api/sources/claude-code/remap").dashboard().send().await.status, 200);
    assert_eq!(num(&totals(&api, &[]).await["prompts"]), before);
}

#[tokio::test]
async fn the_work_split_adds_up_to_the_totals() {
    let api = data().await;
    for (source, cost) in [("repl_main_thread", 0.5), ("agent:builtin:Explore", 0.25), ("compact", 0.125), ("away_summary", 0.125)] {
        api.ingest("claude-code", json!({"hook_event_name": "api_request", "prompt_id": "p2", "session_id": "s1", "model": "opus", "cost_usd": cost, "query_source": source})).await;
    }
    let work = stats(&api, &[("group_by", "work")]).await;
    let cost = groups(&work, "cost_usd");
    assert!((cost["main"] - 3.6).abs() < 1e-9 && cost["subagent"] == 0.25 && cost["system"] == 0.25, "{cost:?}");
    assert!((cost.values().sum::<f64>() - num(&work["totals"]["cost_usd"])).abs() < 1e-9);
    assert_eq!(groups(&work, "prompts")["system"], 1.0); // two system sources, one prompt
    let by_source = stats(&api, &[("group_by", "query_source")]).await;
    let explore = by_source["groups"].as_array().expect("groups").iter().find(|g| g["key"] == "agent:builtin:Explore").expect("group");
    assert_eq!(explore["detail"], "subagent");
    assert_eq!(num(&totals(&api, &[("query_source", "compact")]).await["prompts"]), 1.0);
}

#[tokio::test]
async fn the_text_grouping_gathers_reused_prompts_with_an_excerpt() {
    let api = data().await;
    api.ingest(
        "claude-code",
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": "p5", "session_id": "s4", "prompt": "  fix the LOGIN   bug "}),
    )
    .await;
    let result = stats(&api, &[("group_by", "text"), ("origin", "typed"), ("origin", "slash_command")]).await;
    let top = &result["groups"][0];
    assert_eq!((num(&top["prompts"]), &top["detail"]), (2.0, &json!("fix the LOGIN   bug")));
    assert_eq!(num(&result["totals"]["prompts"]) - num(&result["totals"]["distinct_texts"]), 1.0); // one prompt repeated a text
    assert_eq!(num(&totals(&api, &[("q", "login")]).await["prompts"]), 2.0);
}

#[tokio::test]
async fn a_session_group_has_a_title_and_a_span() {
    let api = data().await;
    let result = stats(&api, &[("group_by", "session")]).await;
    let by_key: BTreeMap<String, &Value> =
        result["groups"].as_array().expect("groups").iter().map(|g| (common::key(&g["key"]), g)).collect();
    assert_eq!(
        (&by_key["s1"]["detail"], num(&by_key["s1"]["span_s"]), num(&by_key["s3"]["span_s"])),
        (&json!("Fix the login bug"), 1830.0, 0.0)
    );
}

#[tokio::test]
async fn groups_can_be_ordered_by_a_measure() {
    let api = data().await;
    let keys = |v: Value| v["groups"].as_array().expect("groups").iter().map(|g| common::key(&g["key"])).collect::<Vec<_>>();
    assert_eq!(keys(stats(&api, &[("group_by", "repo"), ("order", "cost_usd")]).await), ["/r/docs", "/r/app"]);
    assert_eq!(keys(stats(&api, &[("group_by", "session"), ("order", "last_at")]).await), ["s3", "s2", "s1"]);
}

#[tokio::test]
async fn split_by_gives_each_group_its_parts() {
    let api = data().await;
    let parts = |v: &Value, measure: &str| -> BTreeMap<String, BTreeMap<String, f64>> {
        v["groups"]
            .as_array()
            .expect("groups")
            .iter()
            .map(|g| {
                (
                    common::key(&g["key"]),
                    g["parts"].as_array().expect("parts").iter().map(|p| (common::key(&p["key"]), num(&p[measure]))).collect(),
                )
            })
            .collect()
    };
    let by_day = stats(&api, &[("group_by", "repo"), ("split_by", "day")]).await;
    let expected: BTreeMap<String, BTreeMap<String, f64>> =
        [("/r/app", map(&[("2026-11-02", 2.0), ("2026-11-03", 1.0)])), ("/r/docs", map(&[("2026-11-03", 1.0)]))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    assert_eq!(parts(&by_day, "prompts"), expected);
    let kinds = stats(&api, &[("group_by", "repo"), ("split_by", "tool_kind")]).await;
    let expected: BTreeMap<String, BTreeMap<String, f64>> =
        [("/r/app", map(&[("read", 1.0), ("write", 1.0)])), ("/r/docs", map(&[("shell", 1.0)]))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    assert_eq!(parts(&kinds, "tool_calls"), expected);
    let by_outcome = stats(&api, &[("group_by", "outcome"), ("split_by", "repo")]).await;
    let total: f64 = parts(&by_outcome, "prompts").values().flat_map(|p| p.values()).sum();
    assert_eq!(total, 4.0);
}

#[tokio::test]
async fn turn_measures_count_commits_subagents_denials_and_compactions() {
    let api = data().await;
    let events = [
        json!({"hook_event_name": "PostToolUse", "prompt_id": "p1", "session_id": "s1", "tool_name": "Bash", "agent_id": "a1", "tool_input": {"command": "git -C /r/app commit -m fix"}, "duration_ms": 1500}),
        json!({"hook_event_name": "PermissionDenied", "prompt_id": "p3", "session_id": "s2", "tool_name": "Bash"}),
        json!({"hook_event_name": "PreCompact", "prompt_id": "p3", "session_id": "s2"}),
        json!({"hook_event_name": "SubagentStart", "prompt_id": "p1", "session_id": "s1", "agent_type": "Explore"}),
        json!({"hook_event_name": "UserPromptSubmit", "prompt_id": "p6", "session_id": "s5", "prompt": "see https://x.y and @README.md"}),
    ];
    for event in events {
        api.ingest("claude-code", event).await;
    }
    let t = totals(&api, &[]).await;
    let measures = |names: &[&str]| names.iter().map(|n| num(&t[*n])).collect::<Vec<_>>();
    assert_eq!(measures(&["git_commits", "with_commit", "subagent_tool_calls", "subagents"]), [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(measures(&["denials", "compactions", "failed", "files_written", "tool_time_s"]), [1.0, 1.0, 1.0, 1.0, 1.5]);
    assert_eq!(measures(&["with_url", "with_mention", "with_code"]), [1.0, 1.0, 0.0]);
    assert_eq!(num(&t["followed"]), 1.0); // p1 is followed by p2 in s1; every other session has one prompt
}

#[tokio::test]
async fn a_turn_without_an_end_is_superseded_by_a_later_prompt_and_a_new_one_is_running() {
    let api = data().await;
    api.ingest("claude-code", json!({"hook_event_name": "UserPromptSubmit", "prompt_id": "p7", "session_id": "s3", "prompt": "and now?"}))
        .await;
    assert_eq!(
        groups(&stats(&api, &[("group_by", "outcome")]).await, "prompts"),
        map(&[("done", 2.0), ("failed", 1.0), ("superseded", 1.0), ("running", 1.0)])
    );
}

#[tokio::test]
async fn an_interrupt_ends_a_turn_as_interrupted() {
    let api = data().await;
    api.ingest("claude-code", json!({"hook_event_name": "Interrupt", "prompt_id": "p4", "session_id": "s3"})).await;
    assert_eq!(groups(&stats(&api, &[("group_by", "outcome")]).await, "prompts")["interrupted"], 1.0);
    assert_eq!(num(&totals(&api, &[("outcome", "interrupted")]).await["prompts"]), 1.0);
}

// --- telemetry ---

#[tokio::test]
async fn telemetry_is_summed_over_the_sessions_the_filters_keep() {
    let api = data().await;
    let metric = "claude_code.lines_of_code.count";
    for (session, value) in [("s1", 10), ("s2", 5), ("s9", 100)] {
        api.ingest("claude-code", json!({"hook_event_name": metric, "session_id": session, "type": "added", "value": value})).await; // s9 has no prompt
    }
    api.ingest("claude-code", json!({"hook_event_name": "user_prompt", "session_id": "s1", "prompt_id": "p8", "command_name": "model", "command_source": "builtin"})).await;

    let result = api.json_with("/api/stats/telemetry", &[("origin", "typed"), ("origin", "slash_command")]).await;
    assert_eq!((num(&result["sessions"]), num(&result["metrics"][format!("{metric}[added]")])), (2.0, 15.0));
    assert_eq!(result["metrics"].as_object().map(|m| m.len()), Some(1));
    assert_eq!(result["commands"], json!([{"command": "/model", "source": "builtin", "uses": 1, "sessions": 1}]));
    assert_eq!(num(&api.json("/api/stats/telemetry?repo=/r/docs").await["metrics"][format!("{metric}[added]")]), 5.0);
    assert_eq!(api.json_with("/api/stats/telemetry", &[("since", "2099-01-01T00:00:00Z")]).await["metrics"], json!({}));
}
