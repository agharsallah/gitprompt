//! The whole pipeline without a store: redact once, build, render (lib.rs). Ports
//! `tests/export/test_export.py` and the export-logic assertions of `tests/api/test_export_routes.py`
//! (the HTTP routes themselves belong to the server crate): every kind in every format, with
//! secrets, email addresses, account ids, commit authors and home paths planted in the input.
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

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::*;
use gitprompt_export::input::{Chapter, ExportInput, PromptsExport, StoryItem};
use gitprompt_export::{Context, ExportError, Format, Kind, Options, prepare, render, render_text};

fn text(kind: Kind, options: Options, format: Format) -> String {
    render_text(&prepared(kind, options), format, None).expect("render")
}

/// One `#[test]` per (kind, format) case, each calling `$check(kind, format)`.
macro_rules! cases {
    ($check:ident: $($name:ident => ($kind:ident, $format:ident)),* $(,)?) => {
        $( #[test] fn $name() { $check(Kind::$kind, Format::$format) } )*
    };
}

// --- privacy, every kind × every text format -----------------------------------------------------

mod defaults_leave_out_secrets_emails_identity_home_paths_and_text {
    use super::*;

    fn check(kind: Kind, format: Format) {
        assert_absent(&text(kind, Options::default(), format), &never_by_default());
    }

    cases!(check:
        session_md => (Session, Markdown), session_html => (Session, Html), session_csv => (Session, Csv),
        session_json => (Session, Json), session_svg => (Session, Svg),
        project_md => (Project, Markdown), project_html => (Project, Html), project_csv => (Project, Csv),
        project_json => (Project, Json), project_svg => (Project, Svg),
        overview_md => (Overview, Markdown), overview_html => (Overview, Html), overview_csv => (Overview, Csv),
        overview_json => (Overview, Json), overview_svg => (Overview, Svg),
        prompts_md => (Prompts, Markdown), prompts_html => (Prompts, Html), prompts_csv => (Prompts, Csv),
        prompts_json => (Prompts, Json), prompts_svg => (Prompts, Svg),
    );
}

mod with_prompt_text_and_payloads_secrets_emails_and_homes_stay_hidden {
    use super::*;

    fn check(kind: Kind, format: Format) {
        let options = Options { prompt_text: true, drop_raw: false, ..Options::default() };
        assert_absent(&text(kind, options, format), &never_with_text());
    }

    cases!(check:
        session_md => (Session, Markdown), session_html => (Session, Html), session_csv => (Session, Csv),
        session_json => (Session, Json),
        project_md => (Project, Markdown), project_html => (Project, Html), project_csv => (Project, Csv),
        project_json => (Project, Json),
        overview_md => (Overview, Markdown), overview_html => (Overview, Html), overview_csv => (Overview, Csv),
        overview_json => (Overview, Json),
        prompts_md => (Prompts, Markdown), prompts_html => (Prompts, Html), prompts_csv => (Prompts, Csv),
        prompts_json => (Prompts, Json),
    );
}

mod with_prompt_text_the_prompt_shows_with_secrets_emails_and_homes_masked {
    use super::*;

    fn check(kind: Kind, format: Format) {
        let body = text(kind, text_on(), format);
        assert!(body.contains(PLANTED_REDACTED), "{body}");
    }

    cases!(check:
        session_md => (Session, Markdown), session_html => (Session, Html), session_csv => (Session, Csv),
        session_json => (Session, Json),
        project_md => (Project, Markdown), project_html => (Project, Html), project_csv => (Project, Csv),
        project_json => (Project, Json),
        prompts_md => (Prompts, Markdown), prompts_html => (Prompts, Html), prompts_csv => (Prompts, Csv),
        prompts_json => (Prompts, Json),
    );
}

mod with_every_option_opened_secrets_are_still_hidden {
    use super::*;

    fn check(kind: Kind, format: Format) {
        assert_absent(&text(kind, all_open(), format), &never_ever());
    }

    cases!(check:
        session_md => (Session, Markdown), session_html => (Session, Html), session_csv => (Session, Csv),
        session_json => (Session, Json), session_svg => (Session, Svg),
        project_md => (Project, Markdown), project_json => (Project, Json),
        prompts_md => (Prompts, Markdown), prompts_html => (Prompts, Html), prompts_csv => (Prompts, Csv),
        prompts_json => (Prompts, Json),
    );
}

mod the_same_input_renders_the_same_bytes {
    use super::*;

    fn check(kind: Kind, format: Format) {
        pretty_assertions::assert_eq!(text(kind, text_on(), format), text(kind, text_on(), format));
    }

    cases!(check:
        session_md => (Session, Markdown), session_html => (Session, Html), session_csv => (Session, Csv),
        session_json => (Session, Json), session_svg => (Session, Svg),
        project_md => (Project, Markdown), project_html => (Project, Html), project_csv => (Project, Csv),
        project_json => (Project, Json), project_svg => (Project, Svg),
        overview_md => (Overview, Markdown), overview_html => (Overview, Html), overview_csv => (Overview, Csv),
        overview_json => (Overview, Json), overview_svg => (Overview, Svg),
        prompts_md => (Prompts, Markdown), prompts_html => (Prompts, Html), prompts_csv => (Prompts, Csv),
        prompts_json => (Prompts, Json), prompts_svg => (Prompts, Svg),
    );
}

mod html_is_self_contained {
    use super::*;

    fn check(kind: Kind, format: Format) {
        let page = text(kind, text_on(), format);
        let forbidden: Vec<String> = [
            "<script",
            "<link",
            "<img",
            "<iframe",
            "<object",
            "<embed",
            "<video",
            "<audio",
            "<source",
            "<base",
            " src=",
            " href=",
            " srcset=",
            " action=",
            " poster=",
            " data=\"",
            "xlink:href",
            "http://",
            "https://",
            "url(",
            "@import",
        ]
        .map(String::from)
        .to_vec();
        assert_absent(&page, &forbidden);
        assert!(page.contains("default-src 'none'") && page.contains("<svg") && page.contains("prefers-color-scheme: dark"));
        assert!(page.contains("@media print"));
    }

    cases!(check:
        session => (Session, Html), project => (Project, Html), overview => (Overview, Html), prompts => (Prompts, Html),
    );
}

// --- what the defaults keep ----------------------------------------------------------------------

#[test]
fn defaults_keep_figures_and_relative_paths() {
    let md = text(Kind::Session, Options::default(), Format::Markdown);
    assert!(md.contains("# Session s-0001-0"), "{md}");
    assert!(md.contains("`src/app.py`") && md.contains("`~/.zshrc`"), "{md}");
}

#[test]
fn prompt_text_is_quoted_as_typed_and_system_text_as_system_text() {
    let md = text(Kind::Session, text_on(), Format::Markdown);
    assert!(md.contains(&format!("You typed:\n\n```text\n{PLANTED_REDACTED}\n```")), "{md}");
    assert!(md.contains("Task notification:\n\n```text\ndone, ping [hidden email], see ~/Desktop/log.txt\n```"), "{md}");
    assert!(md.starts_with(&format!("# {TITLE}\n")));
}

#[test]
fn session_sections_are_summary_environment_then_the_story() {
    let export = prepared(Kind::Session, Options::default());
    let titles: Vec<&str> = export.report.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Summary", "Environment", "Prompt 1", "1 system item"]);
}

#[test]
fn the_environment_keeps_only_what_is_not_identity() {
    let md = text(Kind::Session, Options::default(), Format::Markdown);
    assert!(md.contains("## Environment\n\n_From telemetry_\n\n- `app.version: 2.1.0`\n\n"), "{md}");
}

#[test]
fn csv_rows_are_one_per_chapter_and_marker() {
    let csv = text(Kind::Session, text_on(), Format::Csv);
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines[0].starts_with("kind,id,turn,received_at,origin,slash_command,branch,outcome,response_s,cost_usd,"), "{csv}");
    assert!(lines[1].starts_with("chapter,7,1,2026-03-02T09:00:00+00:00,typed,,main,done,42.0,0.12,"), "{csv}");
    assert!(lines[2].starts_with("task_notification,8,"), "{csv}");
}

#[test]
fn json_states_its_kind_options_and_rows() {
    let document: Value = serde_json::from_str(&text(Kind::Session, Options::default(), Format::Json)).unwrap();
    assert_eq!(
        (&document["kind"], &document["options"], document["rows"].as_array().map(Vec::len)),
        (&json!("session"), &Options::default().as_json(), Some(2))
    );
}

#[test]
fn json_rows_are_the_exports_rows() {
    let export = prepared(Kind::Project, text_on());
    let document: Value = serde_json::from_str(&render_text(&export, Format::Json, None).unwrap()).unwrap();
    assert_eq!(document["rows"], Value::Array(export.rows.iter().cloned().map(Value::Object).collect()));
}

#[test]
fn markdown_keeps_prompt_markdown_inert() {
    let mut input = session_export();
    input.story.items[0] = StoryItem::Chapter(Chapter { text: Some("![x](https://tracker.invalid/p.png) ```code```".into()), ..chapter() });
    let md = render_text(&prepare(&ExportInput::Session(input), &text_on(), &context()).unwrap(), Format::Markdown, None).unwrap();
    assert!(md.contains("````text\n![x](https://tracker.invalid/p.png) ```code```\n````"), "{md}");
}

#[test]
fn html_escapes_prompt_text() {
    let mut input = session_export();
    input.story.items[0] = StoryItem::Chapter(Chapter { text: Some("<img src=x onerror=alert(1)>".into()), ..chapter() });
    let page = render_text(&prepare(&ExportInput::Session(input), &text_on(), &context()).unwrap(), Format::Html, None).unwrap();
    assert!(!page.contains("<img") && page.contains("&lt;img src=x"), "{page}");
}

// --- each option puts back only what it hides ----------------------------------------------------

#[test]
fn absolute_paths_come_back_with_relative_paths_off() {
    let options = Options { relative_paths: false, ..Options::default() };
    let json = text(Kind::Session, options, Format::Json);
    assert!(json.contains(&format!("{REPO}/src/app.py")), "{json}");
    assert_absent(&json, &[EMAIL.to_string(), AUTHOR.to_string(), SECRET.to_string()]);
}

#[test]
fn identity_comes_back_with_drop_identity_off() {
    let options = Options { drop_identity: false, ..Options::default() };
    let md = text(Kind::Session, options, Format::Markdown);
    assert!(md.contains(ACCOUNT_EMAIL) && md.contains(ORG_ID) && md.contains(AUTHOR), "{md}");
    assert_absent(&md, &[HOME.to_string(), "write to".to_string()]);
}

#[test]
fn stored_payloads_come_back_with_drop_raw_off_with_secrets_still_hidden() {
    let options = Options { drop_raw: false, prompt_text: true, ..Options::default() };
    let json = text(Kind::Prompts, options, Format::Json);
    let document: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(document["rows"][0]["raw"]["hook_event_name"], json!("UserPromptSubmit"));
    assert_eq!(document["rows"][0]["raw"]["transcript_path"], json!("~/.claude/projects/…/s1.jsonl"));
    assert_absent(&json, &never_ever());
}

#[test]
fn stored_payloads_are_left_out_by_default() {
    let export = prepared(Kind::Prompts, Options::default());
    assert!(export.rows.iter().all(|r| !r.contains_key("raw")));
    assert!(export.report.sections.iter().all(|s| s.title != "Stored payloads"));
}

#[test]
fn search_words_are_described_only_with_prompt_text() {
    let mut input = prompts_export();
    input.filters.q = Some("deploy".into());
    let input = ExportInput::Prompts(input);
    let hidden = prepare(&input, &Options::default(), &context()).unwrap();
    let shown = prepare(&input, &text_on(), &context()).unwrap();
    assert_eq!(
        (hidden.report.subtitle.as_str(), shown.report.subtitle.as_str()),
        ("All time · Typed", "All time · Typed · matching “deploy”")
    );
}

// --- context -------------------------------------------------------------------------------------

#[test]
fn overview_project_folders_are_made_relative_without_being_named_as_roots() {
    let md = text(Kind::Overview, Options::default(), Format::Markdown);
    assert!(md.contains("| shop | 2 | 1 | $1.50 | 7 |"), "{md}");
}

#[test]
fn extra_project_folders_from_the_context_are_made_relative() {
    let mut input = session_export();
    input.environment.insert("workspace".into(), "/srv/other/x.txt".into());
    let context = Context { roots: BTreeSet::from(["/srv/other".to_string()]), ..context() };
    let md = render_text(&prepare(&ExportInput::Session(input), &Options::default(), &context).unwrap(), Format::Markdown, None).unwrap();
    assert!(md.contains("- `workspace: x.txt`"), "{md}");
}

#[test]
fn times_read_in_the_contexts_zone() {
    let context = Context { zone: offset(2), ..context() };
    let export = prepare(&planted(Kind::Session), &Options::default(), &context).unwrap();
    assert_eq!(export.report.generated_at, "2026-03-03 12:00");
}

#[test]
fn an_export_records_the_options_that_made_it() {
    assert_eq!(prepared(Kind::Overview, all_open()).options, all_open());
}

// --- single charts -------------------------------------------------------------------------------

#[test]
fn a_session_lists_its_charts_with_data() {
    let ids: Vec<String> =
        gitprompt_export::charts::chart_list(&prepared(Kind::Session, Options::default()).report).into_iter().map(|c| c.id).collect();
    assert_eq!(ids, ["outcomes", "tools"]);
}

#[test]
fn a_single_chart_is_rendered_by_id_and_an_unknown_id_draws_the_first() {
    let export = prepared(Kind::Session, Options::default());
    let tools = render_text(&export, Format::Svg, Some("tools")).unwrap();
    let fallback = render_text(&export, Format::Svg, Some("nope")).unwrap();
    assert!(tools.contains("Tool calls by kind") && fallback.contains("How the turns ended"));
}

#[test]
fn a_chart_format_without_chart_data_is_an_error() {
    let empty = ExportInput::Prompts(PromptsExport { total: 0, prompts: vec![], ..prompts_export() });
    let export = prepare(&empty, &Options::default(), &context()).unwrap();
    assert!(matches!(render(&export, Format::Svg, None), Err(ExportError::NoChart)));
}

#[test]
fn png_renders_bytes_but_is_not_text() {
    let export = prepared(Kind::Session, Options::default());
    assert_eq!(&render(&export, Format::Png, Some("tools")).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    assert!(matches!(render_text(&export, Format::Png, None), Err(ExportError::NotText(Format::Png))));
}

#[test]
fn render_returns_the_text_formats_as_utf8_bytes() {
    let export = prepared(Kind::Prompts, text_on());
    assert_eq!(render(&export, Format::Markdown, None).unwrap(), render_text(&export, Format::Markdown, None).unwrap().into_bytes());
}

// --- kinds and errors ----------------------------------------------------------------------------

#[test]
fn kinds_are_named_by_their_path_segment() {
    assert_eq!(Kind::ALL.map(Kind::as_str), ["session", "project", "overview", "prompts"]);
    assert_eq!(["session", "project", "overview", "prompts"].map(Kind::parse), Kind::ALL.map(Some));
}

#[test]
fn an_unknown_kind_does_not_parse() {
    assert_eq!(["nothing", "Session", ""].map(Kind::parse), [None, None, None]);
}

#[test]
fn export_errors_map_to_domain_errors() {
    use gitprompt_core::Error;
    assert!(matches!(Error::from(ExportError::NoChart), Error::NotFound(m) if m == "this export has no chart with data"));
    assert!(matches!(Error::from(ExportError::NotText(Format::Png)), Error::Invalid(m) if m == "png is not a text format"));
    assert!(matches!(Error::from(ExportError::Store(Error::NotFound("no session".into()))), Error::NotFound(m) if m == "no session"));
    assert!(matches!(Error::from(ExportError::Raster("x".into())), Error::Storage(_)));
}

// --- tool metadata survives the default redaction ------------------------------------------------

#[test]
fn the_tool_column_names_the_tool_with_the_default_options() {
    let project = prepared(Kind::Project, Options::default());
    let md = render_text(&project, Format::Markdown, None).unwrap();
    assert!(md.contains("| 2026-03-02 09:00 | main | Claude Code (Desktop app) | claude-haiku | Done |"), "{md}");
    let prompts = render_text(&prepared(Kind::Prompts, Options::default()), Format::Markdown, None).unwrap();
    assert!(prompts.contains("| shop | Claude Code (Desktop app) | claude-haiku |"), "{prompts}");
}
