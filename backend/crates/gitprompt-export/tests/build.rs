//! Builders (export/build.py): how values read in a report (labels, numbers, times, facts, charts,
//! filters and notes) and which builder each kind of input runs. What each kind of export puts in
//! its report and rows: `build_session.rs`, `build_project.rs`, `build_selection.rs`. Builders are pure, so
//! they get the inputs directly (the redaction pass is tested in redact.rs and through `prepare` in
//! pipeline.rs).
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

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::reading::{bars_of, pairs};
use common::*;
use gitprompt_core::spec::Origin;
use gitprompt_export::build::{self, *};
use gitprompt_export::input::{Connector, Diffstat, ExportInput, Facts, Phase, PhaseReason};
use gitprompt_export::report::{Bar, Chart, ChartKind};
use gitprompt_export::{Kind, Options};
use gitprompt_store::{Has, Outcome, PromptFilter};

// --- labels --------------------------------------------------------------------------------------

#[test]
fn outcomes_read_as_words_and_an_unknown_one_as_is() {
    let read = ["done", "failed", "interrupted", "superseded", "running", "open", "mystery"].map(outcome_label);
    assert_eq!(read, ["Done", "Failed", "Interrupted", "Superseded", "Running", "No end seen", "mystery"]);
}

#[test]
fn origins_read_as_words_and_an_unknown_one_as_is() {
    let read =
        ["typed", "slash_command", "task_notification", "agent_message", "subagent", "programmatic", "unknown", "new"].map(origin_label);
    assert_eq!(read, ["Typed", "Slash command", "Task notification", "Agent message", "Subagent", "Programmatic", "Unknown", "new"]);
}

#[test]
fn markers_read_as_origins_plus_subagent_prompts_and_compactions() {
    let read = ["task_notification", "subagent_prompt", "compaction", "other"].map(marker_label);
    assert_eq!(read, ["Task notification", "Subagent prompt", "Compaction", "other"]);
}

// --- numbers -------------------------------------------------------------------------------------

#[test]
fn integers_are_rounded_with_thousands_separators_and_none_is_zero() {
    let read = [None, Some(12.0), Some(999.4), Some(1_234_567.0)].map(integer);
    assert_eq!(read, ["0", "12", "999", "1,234,567"]);
}

#[test]
fn money_has_two_decimals_and_a_floor_of_a_cent() {
    let read = [None, Some(0.0), Some(0.005), Some(0.01), Some(1234.567)].map(money);
    assert_eq!(read, ["$0.00", "$0.00", "<$0.01", "$0.01", "$1,234.57"]);
}

#[test]
fn a_cost_the_tool_did_not_report_is_a_dash_not_zero() {
    assert_eq!([None, Some(0.0), Some(1.2)].map(reported), ["—", "$0.00", "$1.20"]);
}

#[test]
fn durations_read_in_seconds_minutes_or_hours() {
    let read = [None, Some(42.4), Some(59.6), Some(3599.0), Some(3600.0), Some(3660.0), Some(35_940.0), Some(37_800.0)].map(duration);
    assert_eq!(read, ["—", "42 s", "1 min", "59 min", "1 h", "1 h 1 min", "9 h 59 min", "10 h"]);
}

#[test]
fn plurals_add_an_s_or_the_given_plural_with_separators() {
    let read = [
        plural(1, "commit", None),
        plural(0, "commit", None),
        plural(1200, "file", None),
        plural(1, "entry", Some("entries")),
        plural(2, "entry", Some("entries")),
    ];
    assert_eq!(read, ["1 commit", "0 commits", "1,200 files", "1 entry", "2 entries"]);
}

#[test]
fn changes_read_as_commits_lines_and_files() {
    let big = Diffstat { commits: 2, files: 4, insertions: 1200, deletions: 35 };
    assert_eq!(
        [changes(None), changes(Some(&diffstat())), changes(Some(&big))],
        ["No commits", "1 commit, +3 −0 in 1 file", "2 commits, +1,200 −35 in 4 files"]
    );
}

// --- time ----------------------------------------------------------------------------------------

#[test]
fn an_instant_reads_in_the_readers_time_zone() {
    let read = [
        when(Some("2026-03-02T09:00:00+00:00"), false, utc()),
        when(Some("2026-03-02T09:00:00+00:00"), false, offset(2)),
        when(Some("2026-03-02T03:00:00+00:00"), false, offset(-5)),
        when(Some("2026-03-02T03:00:00+00:00"), true, offset(-5)),
    ];
    assert_eq!(read, ["2026-03-02 09:00", "2026-03-02 11:00", "2026-03-01 22:00", "2026-03-01"]);
}

#[test]
fn an_instant_accepts_z_and_fractional_seconds() {
    let read = [when(Some("2026-03-02T09:00:00Z"), false, utc()), when(Some("2026-03-02T09:00:59.123456+00:00"), false, utc())];
    assert_eq!(read, ["2026-03-02 09:00", "2026-03-02 09:00"]);
}

#[test]
fn a_value_without_an_offset_is_already_local() {
    let read = [when(Some("2026-03-02"), true, offset(-5)), when(Some("2026-03-02T09:00:00"), false, offset(2))];
    assert_eq!(read, ["2026-03-02", "2026-03-02 09:00"]);
}

#[test]
fn a_missing_instant_is_a_dash_and_a_foreign_one_is_shown_as_is() {
    assert_eq!([when(None, false, utc()), when(Some(""), false, utc()), when(Some("yesterday"), false, utc())], ["—", "—", "yesterday"]);
}

#[test]
fn a_span_shows_the_end_time_alone_within_a_day() {
    assert_eq!(span(Some("2026-03-02T09:00:00+00:00"), Some("2026-03-02T09:03:00+00:00"), utc()), "2026-03-02 09:00 – 09:03");
}

#[test]
fn a_span_over_days_shows_both_dates() {
    assert_eq!(span(Some("2026-03-02T09:00:00+00:00"), Some("2026-03-03T10:00:00+00:00"), utc()), "2026-03-02 09:00 – 2026-03-03 10:00");
}

#[test]
fn a_span_without_an_end_or_within_a_minute_is_one_time() {
    let read = [
        span(Some("2026-03-02T09:00:00+00:00"), None, utc()),
        span(Some("2026-03-02T09:00:10+00:00"), Some("2026-03-02T09:00:50+00:00"), utc()),
    ];
    assert_eq!(read, ["2026-03-02 09:00", "2026-03-02 09:00"]);
}

#[test]
fn a_span_without_a_start_is_empty() {
    assert_eq!(span(None, Some("2026-03-02T09:00:00+00:00"), utc()), "");
}

// --- facts ---------------------------------------------------------------------------------------

#[test]
fn the_tool_names_the_app_a_prompt_was_typed_in() {
    assert_eq!(tool(Some(&facts())), "Claude Code (Desktop app)");
}

#[test]
fn the_tool_without_a_surface_is_its_label() {
    let codex = Facts { connector: Some(Connector { label: Some("Codex".into()), ..Connector::default() }), ..Facts::default() };
    assert_eq!(tool(Some(&codex)), "Codex");
}

#[test]
fn the_tool_without_a_connector_label_is_a_dash() {
    assert_eq!([tool(None), tool(Some(&Facts::default()))], ["—", "—"]);
}

#[test]
fn model_and_tokens_are_a_dash_when_not_reported() {
    let read = [model(Some(&facts())), model(None), tokens(Some(&facts())), tokens(Some(&Facts::default())), tokens(None)];
    assert_eq!(read, ["claude-haiku", "—", "1,200", "—", "—"]);
}

#[test]
fn fact_columns_name_the_tool_surface_usage_and_cost_in_order() {
    let columns = Value::Object(fact_columns(Some(&facts())));
    let expected = json!({
        "tool": "claude-code", "surface": "desktop", "model": "claude-haiku", "permission_mode": "default",
        "tool_version": "2.1.0", "api_requests": 2, "tokens": 1200, "cost_usd": 0.5, "reasoning_tokens": 30,
    });
    assert_eq!(serde_json::to_string(&columns).unwrap(), serde_json::to_string(&expected).unwrap());
}

#[test]
fn fact_columns_are_null_where_the_tool_reported_nothing() {
    let bare = Facts { connector: Some(Connector { id: Some("codex".into()), ..Connector::default() }), ..Facts::default() };
    let expected = json!({
        "tool": "codex", "surface": null, "model": null, "permission_mode": null, "tool_version": null,
        "api_requests": null, "tokens": null, "cost_usd": null, "reasoning_tokens": null,
    });
    assert_eq!(Value::Object(fact_columns(Some(&bare))), expected);
}

#[test]
fn there_are_no_fact_columns_without_facts() {
    assert!(fact_columns(None).is_empty());
}

// --- charts --------------------------------------------------------------------------------------

#[test]
fn bars_drop_zero_counts_and_put_the_largest_first_keeping_ties_in_order() {
    let counts = vec![("a".into(), 1.0), ("zero".into(), 0.0), ("b".into(), 5.0), ("c".into(), 1.0)];
    let chart = bars("x", "Title", counts, &|v| format!("{v}!"), "cap");
    assert_eq!(
        chart,
        Chart {
            id: "x".into(),
            title: "Title".into(),
            kind: ChartKind::Bars,
            items: vec![
                Bar { label: "b".into(), value: 5.0, display: "5!".into() },
                Bar { label: "a".into(), value: 1.0, display: "1!".into() },
                Bar { label: "c".into(), value: 1.0, display: "1!".into() },
            ],
            caption: "cap".into(),
        }
    );
}

#[test]
fn bars_keep_the_top_ten() {
    let counts = (1..=12).map(|n| (format!("i{n}"), f64::from(n))).collect();
    let chart = bars("x", "T", counts, &|v| format!("{v}"), "");
    let labels: Vec<&str> = chart.items.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["i12", "i11", "i10", "i9", "i8", "i7", "i6", "i5", "i4", "i3"]);
}

#[test]
fn the_outcome_chart_labels_how_turns_ended() {
    let chart = outcome_chart(&IndexMap::from([("open".to_string(), 1), ("done".to_string(), 3), ("mystery".to_string(), 2)]));
    assert_eq!((chart.id.as_str(), chart.title.as_str()), ("outcomes", "How the turns ended"));
    assert_eq!(bars_of(&chart), pairs(&[("Done", "3"), ("mystery", "2"), ("No end seen", "1")]));
}

// --- filters and notes ---------------------------------------------------------------------------

#[test]
fn no_filters_read_as_all_time() {
    assert_eq!(describe_filters(&PromptFilter::default(), utc()), "All time");
}

#[test]
fn an_open_ended_range_reads_from_the_start_or_to_now() {
    let since = PromptFilter { since: Some("2026-03-01T00:00:00+00:00".into()), ..PromptFilter::default() };
    let until = PromptFilter { until: Some("2026-03-05T00:00:00+00:00".into()), ..PromptFilter::default() };
    assert_eq!([describe_filters(&since, utc()), describe_filters(&until, utc())], ["2026-03-01 to now", "the start to 2026-03-05"]);
}

#[test]
fn every_filter_is_described_in_order() {
    let filters = PromptFilter {
        since: Some("2026-03-01T00:00:00+00:00".into()),
        until: Some("2026-03-05T00:00:00+00:00".into()),
        origin: vec![Origin::Typed, Origin::SlashCommand],
        repo: Some("shop".into()),
        source: Some("codex".into()),
        model: vec!["a".into(), "b".into()],
        tool: Some("Bash".into()),
        tag: Some("wip".into()),
        outcome: vec![Outcome::Done, Outcome::Open],
        has: vec![Has::ToolFailure, Has::NoFollowUp],
        starred: Some(true),
        q: Some("fix login".into()),
        ..PromptFilter::default()
    };
    assert_eq!(
        describe_filters(&filters, utc()),
        "2026-03-01 to 2026-03-05 · Typed, slash command · project shop · source codex · model a, b · tool Bash · tag wip · outcome done, no end seen · with tool failure, no follow up · starred · matching “fix login”"
    );
}

#[test]
fn starred_false_is_not_a_filter() {
    let filters = PromptFilter { starred: Some(false), ..PromptFilter::default() };
    assert_eq!(describe_filters(&filters, utc()), "All time");
}

#[test]
fn default_notes_say_figures_only_relative_paths_and_no_identity() {
    assert_eq!(
        notes(&Options::default(), Kind::Session),
        [
            "Figures only: the text of your prompts, system messages, subagent briefs and session titles is left out.",
            "Secrets that look like keys, tokens or passwords are always replaced by [hidden …].",
            "Paths are relative to the project folder, and your home folder is shown as ~.",
            "Account email, user and organization ids, commit authors and email addresses are left out.",
            "Made by GitPrompt on this computer. Nothing was uploaded.",
        ]
    );
}

#[test]
fn a_prompt_selection_also_notes_its_stored_payloads() {
    assert_eq!(notes(&Options::default(), Kind::Prompts)[4], "Stored hook payloads are left out.");
}

#[test]
fn opened_options_are_noted_as_included() {
    assert_eq!(
        notes(&all_open(), Kind::Prompts),
        [
            "Includes the text you typed (clipped as in the dashboard), and system text quoted as such.",
            "Secrets that look like keys, tokens or passwords are always replaced by [hidden …].",
            "Paths are absolute, as captured.",
            "Includes account identity from telemetry and commit authors.",
            "Includes each prompt's stored hook payload.",
            "Made by GitPrompt on this computer. Nothing was uploaded.",
        ]
    );
}

#[test]
fn a_phase_starts_after_a_pause_on_a_branch_or_at_the_start() {
    let gap = Phase { reason: PhaseReason::Gap, gap_s: Some(10_800.0), ..Phase::default() };
    let gap_unknown = Phase { reason: PhaseReason::Gap, gap_s: None, ..Phase::default() };
    let branch = Phase { reason: PhaseReason::Branch, branches: vec!["feat".into()], ..Phase::default() };
    let branch_unknown = Phase { reason: PhaseReason::Branch, ..Phase::default() };
    let read = [&gap, &gap_unknown, &branch, &branch_unknown, &Phase::default()].map(phase_reason);
    assert_eq!(read, ["After a pause of 3 h", "After a pause of —", "On branch feat", "Branch changed", "Start"]);
}

// --- dispatch ------------------------------------------------------------------------------------

#[test]
fn build_runs_the_builder_of_the_inputs_kind() {
    let options = Options::default();
    assert_eq!(build::build(&ExportInput::Session(session_export()), &options, utc()), build::session(&session_export(), &options, utc()));
    assert_eq!(build::build(&ExportInput::Project(project_export()), &options, utc()), build::project(&project_export(), &options, utc()));
    assert_eq!(
        build::build(&ExportInput::Overview(overview_export()), &options, utc()),
        build::overview(&overview_export(), &options, utc())
    );
    assert_eq!(build::build(&ExportInput::Prompts(prompts_export()), &options, utc()), build::prompts(&prompts_export(), &options, utc()));
}

#[test]
fn an_input_knows_its_kind() {
    let kinds = [Kind::Session, Kind::Project, Kind::Overview, Kind::Prompts].map(|k| planted(k).kind());
    assert_eq!(kinds, Kind::ALL);
}
