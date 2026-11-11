//! Builders (export/build.py): value formatting, and what each kind of export puts in its report
//! and rows. Builders are pure, so they get the inputs directly (the redaction pass is tested in
//! redact.rs and through `prepare` in pipeline.rs).
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

use common::*;
use gitprompt_core::spec::Origin;
use gitprompt_export::build::{self, *};
use gitprompt_export::input::{Chapter, Connector, Diffstat, ExportInput, Facts, Marker, Phase, PhaseReason, SessionExport, StoryItem};
use gitprompt_export::report::{Bar, Block, Chart, ChartKind, Figure, Items, Level, Paragraph, Quote, Report, Section, Table, Voice};
use gitprompt_export::{Kind, Options, Row};
use gitprompt_store::{Has, Outcome, PromptFilter};

fn figures(section: &Section) -> Vec<(String, String)> {
    match &section.blocks[0] {
        Block::Figures(f) => f.items.iter().map(|Figure { label, value }| (label.clone(), value.clone())).collect(),
        other => panic!("not figures: {other:?}"),
    }
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn chart<'a>(report: &'a Report, id: &str) -> &'a Chart {
    report.chart(id).unwrap_or_else(|| panic!("no chart {id}"))
}

fn bars_of(chart: &Chart) -> Vec<(String, String)> {
    chart.items.iter().map(|b| (b.label.clone(), b.display.clone())).collect()
}

fn section<'a>(report: &'a Report, title: &str) -> &'a Section {
    report.sections.iter().find(|s| s.title == title).unwrap_or_else(|| panic!("no section {title}"))
}

fn titles(report: &Report) -> Vec<&str> {
    report.sections.iter().map(|s| s.title.as_str()).collect()
}

fn keys(row: &Row) -> Vec<&str> {
    row.keys().map(String::as_str).collect()
}

fn table(section: &Section) -> &Table {
    section.blocks.iter().find_map(|b| if let Block::Table(t) = b { Some(t) } else { None }).expect("a table")
}

fn session_of(story_edit: impl FnOnce(&mut SessionExport)) -> (Report, Vec<Row>) {
    let mut data = session_export();
    story_edit(&mut data);
    build::session(&data, &Options::default(), utc())
}

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

// --- a session -----------------------------------------------------------------------------------

#[test]
fn a_session_is_titled_by_its_story_title() {
    let (report, _) = session_of(|_| {});
    assert_eq!((report.kind, report.title.as_str()), (Kind::Session, TITLE));
}

#[test]
fn a_session_without_a_title_is_named_by_the_start_of_its_id() {
    let (report, _) = session_of(|d| d.story.title = None);
    assert_eq!(report.title, "Session s-0001-0");
}

#[test]
fn a_session_subtitle_names_project_branches_and_span() {
    let (report, _) = session_of(|d| d.story.branches = vec!["main".into(), "feat".into()]);
    assert_eq!(report.subtitle, "shop · main, feat · 2026-03-02 09:00 – 09:03");
}

#[test]
fn a_session_subtitle_skips_what_is_unknown() {
    let (report, _) = session_of(|d| {
        d.story.project = None;
        d.story.branches = vec![];
        d.story.started_at = None;
    });
    assert_eq!(report.subtitle, "");
}

#[test]
fn a_session_reads_its_export_time_and_notes() {
    let (report, _) = session_of(|_| {});
    assert_eq!(report.generated_at, "2026-03-03 10:00");
    assert_eq!(report.notes, notes(&Options::default(), Kind::Session));
}

#[test]
fn a_session_summary_shows_its_headline_figures() {
    let (report, _) = session_of(|d| d.story.models = IndexMap::from([("opus".into(), 2), ("haiku".into(), 1)]));
    assert_eq!(
        figures(&report.sections[0]),
        pairs(&[
            ("Tool", "claude-code"),
            ("Models", "opus, haiku"),
            ("Prompts you typed", "1"),
            ("System items", "1"),
            ("Span", "3 min"),
            ("Cost", "$0.12"),
            ("Tool calls", "3"),
            ("Files written", "1"),
            ("Retries", "0"),
            ("Changes", "1 commit, +3 −0 in 1 file"),
        ])
    );
}

#[test]
fn a_session_names_its_tool_by_connector_then_source_then_dash() {
    let (named, _) = session_of(|d| d.story.connector = Some(Connector { name: Some("Claude Code".into()), ..Connector::default() }));
    let (unnamed, _) = session_of(|d| d.story.source = None);
    assert_eq!(figures(&named.sections[0])[0].1, "Claude Code");
    assert_eq!(figures(&unnamed.sections[0])[0].1, "—");
}

#[test]
fn a_session_without_cost_models_or_end_leaves_cost_out_and_shows_dashes() {
    let (report, _) = session_of(|d| {
        d.story.totals.cost_usd = 0.0;
        d.story.ended_at = None;
    });
    let shown = figures(&report.sections[0]);
    assert!(shown.iter().all(|(label, _)| label != "Cost"), "{shown:?}");
    assert_eq!((shown[1].1.as_str(), shown[4].1.as_str()), ("—", "—"));
}

#[test]
fn a_session_charts_outcomes_and_tool_calls_by_kind_over_all_chapters() {
    let (report, _) = session_of(|d| {
        let second = Chapter { id: 9, turn: 3, tools: IndexMap::from([("shell".into(), 4), ("read".into(), 2)]), ..chapter() };
        d.story.items.push(StoryItem::Chapter(second));
    });
    assert_eq!(bars_of(chart(&report, "outcomes")), pairs(&[("Done", "1")]));
    assert_eq!(bars_of(chart(&report, "tools")), pairs(&[("shell", "4"), ("read", "3"), ("write", "2")]));
    assert_eq!(chart(&report, "tools").title, "Tool calls by kind");
}

#[test]
fn a_session_lists_its_telemetry_environment_sorted() {
    let (report, _) =
        session_of(|d| d.environment = IndexMap::from([("os.type".into(), "darwin".into()), ("app.version".into(), "2.1.0".into())]));
    let env = section(&report, "Environment");
    assert_eq!(env.subtitle, "From telemetry");
    assert_eq!(env.blocks, vec![Block::Items(Items { items: vec!["app.version: 2.1.0".into(), "os.type: darwin".into()], mono: true })]);
}

#[test]
fn a_session_without_environment_has_no_environment_section() {
    let (report, _) = session_of(|d| d.environment = IndexMap::new());
    assert_eq!(titles(&report), ["Summary", "Prompt 1", "1 system item"]);
}

#[test]
fn a_chapter_is_a_level_three_section_with_its_facts_as_subtitle() {
    let (report, _) = session_of(|_| {});
    let prompt = section(&report, "Prompt 1");
    assert_eq!(prompt.level, Level::Three);
    assert_eq!(prompt.subtitle, "2026-03-02 09:00 · Done · answered in 42 s · 3 tool calls · 1 file written");
}

#[test]
fn a_chapter_subtitle_adds_model_tokens_cost_and_retry_when_known() {
    let (report, _) = session_of(|d| {
        let c = Chapter { facts: Some(facts()), retry_of: Some(5), tool_calls: 1, files_written: 0, response_s: None, ..chapter() };
        d.story.items = vec![StoryItem::Chapter(c)];
    });
    assert_eq!(
        section(&report, "Prompt 1").subtitle,
        "2026-03-02 09:00 · Done · claude-haiku · 1,200 tokens · $0.50 · 1 tool call · retry of #5"
    );
}

#[test]
fn a_chapter_quotes_what_you_typed_then_lists_files_and_commits() {
    let (report, _) = session_of(|d| {
        d.story.items = vec![StoryItem::Chapter(Chapter { text: Some("hello".into()), ..chapter() })];
    });
    assert_eq!(
        section(&report, "Prompt 1").blocks,
        vec![
            Block::Quote(Quote { text: "hello".into(), voice: Voice::You, caption: String::new() }),
            Block::Items(Items { items: vec![format!("{REPO}/src/app.py"), format!("{HOME}/.zshrc")], mono: true }),
            Block::Items(Items { items: vec!["abc1234 Add app (+3 −0) — Jane Doe".into()], mono: true }),
        ]
    );
}

#[test]
fn a_clipped_chapter_says_so_and_a_commit_without_author_has_no_dash() {
    let (report, _) = session_of(|d| {
        let mut c = Chapter { text: Some("hel…".into()), clipped: true, files: vec![], ..chapter() };
        c.commits[0].author = None;
        d.story.items = vec![StoryItem::Chapter(c)];
    });
    assert_eq!(
        section(&report, "Prompt 1").blocks,
        vec![
            Block::Quote(Quote { text: "hel…".into(), voice: Voice::You, caption: "clipped".into() }),
            Block::Items(Items { items: vec!["abc1234 Add app (+3 −0)".into()], mono: true }),
        ]
    );
}

#[test]
fn a_chapter_without_text_files_or_commits_has_no_blocks() {
    let (report, _) = session_of(|d| {
        let c = Chapter { text: None, files: vec![], commits: vec![], slash_command: Some("/review".into()), ..chapter() };
        d.story.items = vec![StoryItem::Chapter(c)];
    });
    assert_eq!(section(&report, "Prompt 1 · /review").blocks, vec![]);
}

#[test]
fn markers_are_one_section_counting_them_as_not_typed_by_you() {
    let (report, _) = session_of(|d| {
        let compaction = Marker { kind: "compaction".into(), id: 9, excerpt: None, ..marker() };
        let brief =
            Marker { kind: "subagent_prompt".into(), id: 10, label: Some("Look".into()), excerpt: Some("check x".into()), ..marker() };
        d.story.items = vec![StoryItem::Markers { items: vec![marker(), marker(), compaction, brief] }];
    });
    let group = &report.sections[2];
    assert_eq!((group.title.as_str(), group.subtitle.as_str(), group.level), ("4 system items", "2026-03-02 09:02", Level::Three));
    assert_eq!(
        group.blocks[0],
        Block::Paragraph(Paragraph {
            text: "2 task notifications, 1 compaction, 1 subagent prompt: not typed by you.".into(),
            muted: true
        })
    );
    assert_eq!(
        group.blocks[3],
        Block::Quote(Quote { text: "check x".into(), voice: Voice::System, caption: "Subagent prompt · Look".into() })
    );
    assert_eq!(group.blocks.len(), 4);
}

#[test]
fn a_marker_excerpt_is_quoted_as_system_text_by_its_kind() {
    let (report, _) = session_of(|_| {});
    assert_eq!(
        section(&report, "1 system item").blocks[1],
        Block::Quote(Quote { text: marker().excerpt.unwrap(), voice: Voice::System, caption: "Task notification".into() })
    );
}

#[test]
fn session_rows_are_one_per_chapter_and_per_marker() {
    let (_, rows) = session_of(|_| {});
    assert_eq!(rows.len(), 2);
    assert_eq!(
        keys(&rows[0]),
        [
            "kind",
            "id",
            "turn",
            "received_at",
            "origin",
            "slash_command",
            "branch",
            "outcome",
            "response_s",
            "cost_usd",
            "tokens",
            "tool_calls",
            "tool_failures",
            "files_written",
            "subagents",
            "git_commits",
            "retry_of",
            "commits",
            "insertions",
            "deletions",
            "files",
        ]
    );
    assert_eq!(rows[0]["files"], json!([format!("{REPO}/src/app.py"), format!("{HOME}/.zshrc")]));
    assert_eq!((&rows[0]["kind"], &rows[0]["cost_usd"], &rows[0]["commits"]), (&json!("chapter"), &json!(0.12), &json!(1)));
}

#[test]
fn a_marker_row_keeps_only_the_texts_it_has() {
    let (_, rows) = session_of(|_| {});
    assert_eq!(
        Value::Object(rows[1].clone()),
        json!({"kind": "task_notification", "id": 8, "received_at": "2026-03-02T09:02:00+00:00", "excerpt": marker().excerpt})
    );
}

#[test]
fn a_chapter_row_holds_its_text_only_with_prompt_text() {
    let data = session_export();
    let (_, with_text) = build::session(&data, &text_on(), utc());
    let mut no_text = data;
    no_text.story.items = vec![StoryItem::Chapter(Chapter { text: None, ..chapter() })];
    let (_, null_text) = build::session(&no_text, &text_on(), utc());
    assert_eq!(with_text[0]["text"], json!(planted_text()));
    assert_eq!(null_text[0]["text"], Value::Null);
}

#[test]
fn a_chapter_row_with_facts_takes_cost_and_tokens_from_them_in_place() {
    let (_, rows) = session_of(|d| d.story.items = vec![StoryItem::Chapter(Chapter { facts: Some(facts()), ..chapter() })]);
    let row = &rows[0];
    assert_eq!((&row["cost_usd"], &row["tokens"], &row["surface"]), (&json!(0.5), &json!(1200), &json!("desktop")));
    assert_eq!(&keys(row)[9..11], ["cost_usd", "tokens"]);
    assert_eq!(keys(row).last(), Some(&"reasoning_tokens"));
}

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
