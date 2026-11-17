//! The session export's report and rows (export/build.py `build_session`): title, summary, charts,
//! environment, one section per chapter, the markers, and one row per chapter and marker.
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

use common::reading::{bars_of, chart, figures, keys, pairs, section, titles};
use common::*;
use gitprompt_export::build::{self, *};
use gitprompt_export::input::{Chapter, Connector, Marker, SessionExport, StoryItem};
use gitprompt_export::report::{Block, Items, Level, Paragraph, Quote, Report, Voice};
use gitprompt_export::{Kind, Options, Row};

fn session_of(story_edit: impl FnOnce(&mut SessionExport)) -> (Report, Vec<Row>) {
    let mut data = session_export();
    story_edit(&mut data);
    build::session(&data, &Options::default(), utc())
}

// --- a session -----------------------------------------------------------------------------------

#[test]
fn a_session_is_titled_by_its_story_label() {
    let (report, _) = session_of(|_| {});
    assert_eq!((report.kind, report.title.as_str()), (Kind::Session, TITLE));
}

#[test]
fn a_session_you_named_is_titled_by_its_name() {
    let (report, _) = session_of(|d| d.story.name = Some("My name".into()));
    assert_eq!(report.title, "My name");
}

#[test]
fn a_session_without_a_title_is_named_by_the_start_of_its_id() {
    let (report, _) = session_of(|d| d.story.label = None);
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
