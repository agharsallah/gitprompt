//! The exports of the prompts a filter selects (export/build.py `build_overview`, `build_prompts`):
//! the Overview's figures, charts, projects and statistics rows, and a prompt selection's table,
//! payloads and rows.
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
use serde_json::{Value, json};

use common::reading::{bars_of, chart, figures, keys, pairs, section, table, titles};
use common::*;
use gitprompt_export::build::{self, *};
use gitprompt_export::report::{Block, ChartKind, Report, Table};
use gitprompt_export::{Kind, Options, Row};

// --- the overview --------------------------------------------------------------------------------

fn overview_report() -> (Report, Vec<Row>) {
    build::overview(&overview_export(), &Options::default(), utc())
}

#[test]
fn the_overview_is_titled_overview_and_describes_its_filters() {
    let (report, _) = overview_report();
    assert_eq!(
        (report.kind, report.title.as_str(), report.subtitle.as_str()),
        (Kind::Overview, "Overview", "2026-03-01 to now · Typed, slash command")
    );
}

#[test]
fn the_overview_figures_include_cost_and_tokens_when_reported() {
    let (report, _) = overview_report();
    assert_eq!(
        figures(&report.sections[0]),
        pairs(&[
            ("Prompts", "3"),
            ("Sessions", "2"),
            ("Cost", "$1.50"),
            ("Tokens", "1,500"),
            ("Tool calls", "7"),
            ("Failed turns", "1"),
            ("Median response", "42 s"),
            ("Commits by the agent", "2"),
        ])
    );
}

#[test]
fn the_overview_leaves_out_cost_and_tokens_no_tool_reported() {
    let mut data = overview_export();
    data.totals.cost_usd = 0.0;
    data.totals.api_requests = 0.0;
    data.totals.response_s_p50 = None;
    let (report, _) = build::overview(&data, &Options::default(), utc());
    let labels: Vec<String> = figures(&report.sections[0]).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, ["Prompts", "Sessions", "Tool calls", "Failed turns", "Median response", "Commits by the agent"]);
    assert_eq!(figures(&report.sections[0])[4].1, "—");
}

#[test]
fn the_overview_sections_and_charts_come_in_dashboard_order() {
    let (report, _) = overview_report();
    assert_eq!(titles(&report), ["Summary", "Prompt types", "Models and cost", "Agentic work", "Projects"]);
    let ids: Vec<&str> = report.charts().into_iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["activity", "origins", "commands", "models", "cost", "tool-kinds", "outcomes"]);
}

#[test]
fn the_activity_chart_is_prompts_per_day_as_columns() {
    let (report, _) = overview_report();
    let activity = chart(&report, "activity");
    assert_eq!((activity.title.as_str(), activity.kind), ("Prompts per day", ChartKind::Columns));
    assert_eq!(bars_of(activity), pairs(&[("2026-03-01", "1"), ("2026-03-02", "2")]));
}

#[test]
fn overview_charts_label_their_keys_and_skip_the_none_key() {
    let (report, _) = overview_report();
    assert_eq!(bars_of(chart(&report, "origins")), pairs(&[("Typed", "2"), ("Task notification", "1")]));
    assert_eq!(bars_of(chart(&report, "commands")), pairs(&[("/review", "1")]));
    assert_eq!(bars_of(chart(&report, "models")), pairs(&[("claude-haiku", "2")]));
    assert_eq!(bars_of(chart(&report, "cost")), pairs(&[("claude-haiku", "$1.50")]));
    assert_eq!(bars_of(chart(&report, "tool-kinds")), pairs(&[("write", "5"), ("read", "2")]));
    assert_eq!(bars_of(chart(&report, "outcomes")), pairs(&[("Done", "2"), ("Failed", "1")]));
}

#[test]
fn the_projects_table_names_a_project_by_its_remote_else_its_folder() {
    let mut data = overview_export();
    data.groups.repo.push(bucket(None, 1));
    let (report, _) = build::overview(&data, &Options::default(), utc());
    assert_eq!(
        table(section(&report, "Projects")),
        &Table {
            columns: ["Project", "Prompts", "Sessions", "Cost", "Tool calls"].map(String::from).to_vec(),
            rows: vec![
                [REPO, "2", "1", "$1.50", "7"].map(String::from).to_vec(),
                ["acme/api", "1", "1", "$0.00", "0"].map(String::from).to_vec(),
                ["None", "1", "1", "$0.00", "0"].map(String::from).to_vec(),
            ],
            numeric: vec![false, true, true, true, true],
        }
    );
}

#[test]
fn overview_rows_are_a_total_then_every_bucket_of_every_grouping() {
    let (_, rows) = overview_report();
    let groupings: Vec<&str> = rows.iter().map(|r| r["grouping"].as_str().unwrap_or("<not a string>")).collect();
    assert_eq!(
        groupings,
        [
            "total",
            "day",
            "day",
            "origin",
            "origin",
            "model",
            "model",
            "tool_kind",
            "tool_kind",
            "outcome",
            "outcome",
            "outcome",
            "repo",
            "repo",
            "command",
            "command"
        ]
    );
}

#[test]
fn an_overview_total_row_has_every_bucket_column_with_whole_numbers_as_integers() {
    let (_, rows) = overview_report();
    assert_eq!(
        serde_json::to_string(&rows[0]).unwrap(),
        serde_json::to_string(&json!({
            "grouping": "total", "key": null, "prompts": 3, "sessions": 2, "cost_usd": 1.5, "input_tokens": 1000,
            "output_tokens": 500, "tool_calls": 7, "tool_failures": 0, "failed": 1, "files_written": 0, "git_commits": 2,
            "subagents": 0, "response_s_p50": 42, "response_s_p90": null, "first_at": null, "last_at": null,
        }))
        .unwrap()
    );
}

#[test]
fn an_overview_bucket_row_names_its_key() {
    let (_, rows) = overview_report();
    assert_eq!(&keys(&rows[3])[..4], ["grouping", "key", "name", "prompts"]);
    assert_eq!((&rows[3]["key"], &rows[3]["name"]), (&json!("typed"), &json!("Typed")));
    assert_eq!((&rows[6]["key"], &rows[6]["name"]), (&Value::Null, &json!("None")));
}

// --- a prompt selection --------------------------------------------------------------------------

fn prompts_report(options: Options) -> (Report, Vec<Row>) {
    build::prompts(&prompts_export(), &options, utc())
}

#[test]
fn a_prompt_selection_is_titled_prompts_and_describes_its_filters() {
    let (report, _) = prompts_report(Options::default());
    assert_eq!((report.kind, report.title.as_str(), report.subtitle.as_str()), (Kind::Prompts, "Prompts", "All time · Typed"));
}

#[test]
fn a_prompt_selection_summary_counts_prompts_sessions_cost_and_tool_calls() {
    let (report, _) = prompts_report(Options::default());
    assert_eq!(figures(&report.sections[0]), pairs(&[("Prompts", "2 prompts"), ("Sessions", "1"), ("Cost", "$0.50"), ("Tool calls", "2")]));
}

#[test]
fn a_cut_selection_states_the_total_and_a_free_one_leaves_cost_out() {
    let mut data = prompts_export();
    data.total = 2500;
    data.prompts.remove(0);
    let (report, _) = build::prompts(&data, &Options::default(), utc());
    assert_eq!(
        figures(&report.sections[0]),
        pairs(&[("Prompts", "1 prompt of 2,500, the newest"), ("Sessions", "1"), ("Tool calls", "0")])
    );
}

#[test]
fn a_prompt_selection_charts_outcomes_and_origins() {
    let (report, _) = prompts_report(Options::default());
    assert_eq!(bars_of(chart(&report, "outcomes")), pairs(&[("Done", "1"), ("No end seen", "1")]));
    assert_eq!(bars_of(chart(&report, "origins")), pairs(&[("Typed", "1"), ("Task notification", "1")]));
}

#[test]
fn a_prompt_table_reads_each_prompt_and_adds_its_text_with_prompt_text() {
    let mut data = prompts_export();
    data.prompts[1].prompt.row.repo = None;
    data.prompts[1].prompt.row.typed_text = None;
    let (report, _) = build::prompts(&data, &text_on(), utc());
    let t = table(section(&report, "Prompts"));
    assert_eq!(
        t.columns,
        ["When", "Project", "Tool", "Model", "Origin", "Outcome", "Tokens", "Cost", "Tool calls", "Prompt"].map(String::from).to_vec()
    );
    assert_eq!(
        t.rows,
        vec![
            [
                "2026-03-02 09:01",
                REPO,
                "Claude Code (Desktop app)",
                "claude-haiku",
                "Typed",
                "Done",
                "1,200",
                "$0.50",
                "2",
                planted_text().as_str()
            ]
            .map(String::from)
            .to_vec(),
            ["2026-03-02 09:02", "—", "—", "—", "Task notification", "No end seen", "—", "—", "0", ""].map(String::from).to_vec(),
        ]
    );
    assert_eq!(t.numeric, vec![false, false, false, false, false, false, true, true, true, false]);
}

#[test]
fn a_prompt_is_named_by_its_remote_before_its_folder() {
    let mut data = prompts_export();
    data.prompts[0].prompt.row.repo_name = Some("acme/shop".into());
    let (report, _) = build::prompts(&data, &Options::default(), utc());
    assert_eq!(table(section(&report, "Prompts")).rows[0][1], "acme/shop");
    assert_eq!(table(section(&report, "Prompts")).columns.len(), 9);
}

#[test]
fn stored_payloads_are_listed_as_compact_json_clipped_to_raw_chars() {
    let mut data = prompts_export();
    data.prompts[0].raw = Some(json!({"t": "é".repeat(3000)}));
    data.prompts[1].raw = Some(json!({"hook_event_name": "Stop"}));
    let (report, _) = build::prompts(&data, &Options::default(), utc());
    let Block::Items(items) = &section(&report, "Stored payloads").blocks[0] else { panic!("not items") };
    assert!(items.mono);
    assert_eq!(items.items[0].chars().count(), "#1 ".len() + RAW_CHARS);
    assert_eq!(items.items[1], r#"#2 {"hook_event_name":"Stop"}"#);
}

#[test]
fn there_is_no_payload_section_without_a_non_empty_payload() {
    let mut data = prompts_export();
    data.prompts[0].raw = Some(json!({}));
    let (report, _) = build::prompts(&data, &Options::default(), utc());
    assert_eq!(titles(&report), ["Summary", "Prompts"]);
}

#[test]
fn prompt_rows_hold_the_prompt_columns_then_facts_then_text_and_payload() {
    let (_, rows) = prompts_report(text_on());
    let mut expected: Vec<&str> = PROMPT_COLUMNS.to_vec();
    expected.extend([
        "tool",
        "surface",
        "model",
        "permission_mode",
        "tool_version",
        "api_requests",
        "tokens",
        "reasoning_tokens",
        "typed_text",
        "raw",
    ]);
    assert_eq!(keys(&rows[0]), expected);
    assert_eq!((&rows[0]["outcome"], &rows[0]["cost_usd"], &rows[0]["raw"]), (&json!("done"), &json!(0.5), &raw_payload()));
}

#[test]
fn a_prompt_row_without_facts_text_option_or_payload_has_only_the_prompt_columns() {
    let (_, rows) = prompts_report(Options::default());
    assert_eq!(keys(&rows[1]), PROMPT_COLUMNS.to_vec());
    assert_eq!(rows[1]["outcome"], json!("open"));
}
