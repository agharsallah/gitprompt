//! The project export's report and rows (export/build.py `build_project`): title, measures, charts,
//! one section per phase with its prompts and commits, and one row per prompt.
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

use common::reading::{bars_of, chart, figures, keys, pairs, table, titles};
use common::*;
use gitprompt_export::build::{self};
use gitprompt_export::report::{Block, Items, Paragraph, Report, Table};
use gitprompt_export::{Kind, Options, Row};

// --- a project -----------------------------------------------------------------------------------

fn project_with(options: Options) -> (Report, Vec<Row>) {
    let mut data = project_export();
    data.story.repo = Some("shop".into());
    build::project(&data, &options, utc())
}

#[test]
fn a_project_is_titled_by_its_name_with_folder_and_span() {
    let (report, _) = project_with(Options::default());
    assert_eq!((report.kind, report.title.as_str(), report.subtitle.as_str()), (Kind::Project, "shop", "shop · 2026-03-02 09:00 – 13:00"));
}

#[test]
fn a_project_without_name_or_repo_is_just_project() {
    let mut data = project_export();
    data.story.name = None;
    data.story.repo = None;
    data.story.totals.first_at = None;
    let (report, _) = build::project(&data, &Options::default(), utc());
    assert_eq!((report.title.as_str(), report.subtitle.as_str()), ("Project", ""));
}

#[test]
fn a_project_summary_shows_its_measures() {
    let (report, _) = project_with(Options::default());
    assert_eq!(
        figures(&report.sections[0]),
        pairs(&[
            ("Prompts", "2"),
            ("Sessions", "1"),
            ("Cost", "$0.50"),
            ("Tool calls", "3"),
            ("Files written", "1"),
            ("Retries", "0"),
            ("Median length", "120 characters"),
            ("Changes", "1 commit, +3 −0 in 1 file"),
        ])
    );
}

#[test]
fn a_project_charts_prompts_per_phase_and_outcomes_and_counts_other_origins() {
    let (report, _) = project_with(Options::default());
    assert_eq!(bars_of(chart(&report, "phases")), pairs(&[("Phase 1", "1"), ("Phase 2", "1")]));
    assert_eq!(bars_of(chart(&report, "outcomes")), pairs(&[("Done", "2")]));
    assert_eq!(
        report.sections[0].blocks[3],
        Block::Paragraph(Paragraph { text: "2 prompts of other origins in this project are counted, not shown.".into(), muted: true })
    );
}

#[test]
fn each_phase_is_a_section_with_its_reason_and_span() {
    let (report, _) = project_with(Options::default());
    assert_eq!(titles(&report), ["Summary", "Phase 1", "Phase 2"]);
    assert_eq!(
        [report.sections[1].subtitle.as_str(), report.sections[2].subtitle.as_str()],
        ["Start · 2026-03-02 09:00", "After a pause of 4 h · 2026-03-02 13:00"]
    );
}

#[test]
fn a_phase_tables_its_prompts_without_text_by_default() {
    let (report, _) = project_with(Options::default());
    assert_eq!(
        table(&report.sections[1]),
        &Table {
            columns: ["When", "Branch", "Tool", "Model", "Outcome", "Cost", "Tools", "Commits"].map(String::from).to_vec(),
            rows: vec![
                ["2026-03-02 09:00", "main", "Claude Code (Desktop app)", "claude-haiku", "Done", "$0.50", "write 2, read 1", "1"]
                    .map(String::from)
                    .to_vec()
            ],
            numeric: vec![false, false, false, false, false, true, false, true, false],
        }
    );
}

#[test]
fn a_phase_table_shows_dashes_for_what_is_unknown_and_the_prompt_with_prompt_text() {
    let mut data = project_export();
    data.story.phases[1].items[0].text = None;
    let (report, _) = build::project(&data, &text_on(), utc());
    let t = table(&report.sections[2]);
    assert_eq!(t.columns.last().map(String::as_str), Some("Prompt"));
    assert_eq!(t.rows[0], ["2026-03-02 13:00", "—", "—", "—", "Done", "—", "—", "0", ""].map(String::from).to_vec());
}

#[test]
fn a_phase_lists_its_commits_and_a_phase_without_commits_does_not() {
    let (report, _) = project_with(Options::default());
    assert_eq!(
        report.sections[1].blocks.last(),
        Some(&Block::Items(Items { items: vec!["abc1234 Add app (+3 −0) — Jane Doe".into()], mono: true }))
    );
    assert_eq!(report.sections[2].blocks.len(), 2);
}

#[test]
fn project_rows_are_prompts_with_their_phase_tools_commit_count_and_facts() {
    let (_, rows) = project_with(Options::default());
    assert_eq!(rows.len(), 2);
    assert_eq!(
        keys(&rows[0]),
        [
            "phase",
            "id",
            "received_at",
            "session",
            "origin",
            "branch",
            "chars",
            "outcome",
            "response_s",
            "cost_usd",
            "tool_calls",
            "tool_failures",
            "files_written",
            "git_commits",
            "retry_of",
            "tools",
            "commits",
            "tool",
            "surface",
            "model",
            "permission_mode",
            "tool_version",
            "api_requests",
            "tokens",
            "reasoning_tokens",
        ]
    );
    assert_eq!((&rows[0]["phase"], &rows[0]["tools"], &rows[0]["commits"]), (&json!(1), &json!({"write": 2, "read": 1}), &json!(1)));
    assert_eq!((&rows[1]["phase"], rows[1].contains_key("tool")), (&json!(2), false));
}

#[test]
fn a_project_row_holds_its_text_only_with_prompt_text() {
    let (_, rows) = project_with(text_on());
    assert_eq!(keys(&rows[0])[6], "text");
    assert_eq!(rows[1]["text"], json!("tidy up"));
}
