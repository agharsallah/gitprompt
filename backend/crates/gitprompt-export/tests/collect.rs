//! Collecting an export from a store (lib.rs `collect`, `build_from_store`): an in-memory store with
//! a few indexed prompts, a secret and a home path planted in what was typed. No git repository is
//! involved (the planted repo folder doesn't exist, so a story reads no commits).

mod common;

use std::sync::Arc;

use jiff::Timestamp;
use pretty_assertions::assert_eq;
use serde_json::json;

use common::*;
use gitprompt_core::{Error, JsonMap};
use gitprompt_export::input::{ExportInput, StoryItem};
use gitprompt_export::{ExportError, Format, Options, Request, build_from_store, collect, render_text};
use gitprompt_store::index::{Entry, IndexRow, UsageTotals};
use gitprompt_store::{PromptFilter, Store};

const NOW: &str = "2026-03-03T10:00:00+00:00";

/// A store whose clock is a day after the prompts (none is still running).
fn store() -> Store {
    let now: Timestamp = NOW.parse().expect("a timestamp");
    Store::open_in_memory().expect("an in-memory store").with_clock(Arc::new(move || now))
}

/// Store a prompt and its index row (as the indexer would write it).
fn add(store: &Store, row: IndexRow) -> i64 {
    let fields: JsonMap = json!({"event": "UserPromptSubmit", "session_id": row.session, "prompt_id": row.prompt_id})
        .as_object()
        .cloned()
        .expect("an object");
    let raw = json!({"hook_event_name": "UserPromptSubmit", "prompt": row.typed_text, "cwd": row.repo});
    let id = store
        .add_prompt_at(&row.source, &fields, &JsonMap::new(), &raw, &row.received_at)
        .expect("add a prompt")
        .expect("not a duplicate")
        .id;
    store.index().upsert(&[Entry { row: IndexRow { id, ..row }, ..Entry::default() }]).expect("index it");
    id
}

fn row(n: i64, origin: &str, typed: &str) -> IndexRow {
    IndexRow {
        prompt_id: Some(format!("p{n}")),
        source: "claude-code".into(),
        session: Some("s1".into()),
        repo: Some(REPO.into()),
        branch: Some("main".into()),
        received_at: format!("2026-03-02T09:0{n}:00+00:00"),
        day: "2026-03-02".into(),
        hour: 9,
        weekday: 1,
        origin: origin.into(),
        chars: typed.chars().count() as i64,
        typed_text: Some(typed.into()),
        ended_by: Some("Stop".into()),
        response_s: Some(30.0),
        usage: UsageTotals { api_requests: 1, cost_usd: 0.25, input_tokens: 100, output_tokens: 20, ..UsageTotals::default() },
        ..IndexRow::default()
    }
}

/// Two prompts you typed and a task notification between them, in session s1 of the shop repo.
fn planted() -> Store {
    let store = store();
    add(&store, row(1, "typed", &planted_text()));
    add(&store, row(2, "task_notification", &format!("done, ping {EMAIL}")));
    add(&store, row(3, "typed", "tidy up"));
    store
}

fn not_found(result: Result<ExportInput, ExportError>) -> Option<String> {
    match result {
        Err(ExportError::Store(Error::NotFound(message))) => Some(message),
        _ => None,
    }
}

// --- a session -----------------------------------------------------------------------------------

#[test]
fn a_session_is_collected_as_its_story_with_chapters_and_markers() {
    let Ok(ExportInput::Session(session)) = collect(&planted(), &Request::Session { id: "s1".into() }, false, NOW) else {
        panic!("a session export")
    };
    assert_eq!((session.generated_at.as_str(), session.story.session.as_str()), (NOW, "s1"));
    let kinds: Vec<&str> = session
        .story
        .items
        .iter()
        .map(|item| match item {
            StoryItem::Chapter(_) => "chapter",
            StoryItem::Markers { .. } => "markers",
        })
        .collect();
    assert_eq!(kinds, ["chapter", "markers", "chapter"]);
    assert_eq!((session.story.totals.chapters, session.story.totals.markers), (2, 1));
}

#[test]
fn an_unknown_session_is_not_found() {
    assert_eq!(not_found(collect(&planted(), &Request::Session { id: "nope".into() }, false, NOW)).as_deref(), Some("session not found"));
}

#[test]
fn a_session_built_from_the_store_keeps_the_defaults_private() {
    let export = build_from_store(&planted(), &Request::Session { id: "s1".into() }, &Options::default(), &context(), NOW).unwrap();
    let md = render_text(&export, Format::Markdown, None).unwrap();
    assert!(md.contains("### Prompt 1") && md.contains("### Prompt 3"), "{md}");
    assert!(md.contains("Exported from GitPrompt on 2026-03-03 10:00."), "{md}");
    assert_absent(&md, &never_by_default());
}

#[test]
fn a_session_built_with_prompt_text_shows_it_redacted() {
    let export = build_from_store(&planted(), &Request::Session { id: "s1".into() }, &text_on(), &context(), NOW).unwrap();
    let md = render_text(&export, Format::Markdown, None).unwrap();
    assert!(md.contains(PLANTED_REDACTED), "{md}");
    assert_absent(&md, &never_with_text());
}

// --- a project -----------------------------------------------------------------------------------

#[test]
fn a_project_is_collected_by_its_project_id_counting_other_origins() {
    let id = gitprompt_insights::stories::project_id(REPO);
    let Ok(ExportInput::Project(project)) = collect(&planted(), &Request::Project { id }, false, NOW) else { panic!("a project export") };
    assert_eq!((project.story.repo.as_deref(), project.story.name.as_deref(), project.story.system), (Some(REPO), Some("shop"), 1));
    assert_eq!(project.story.totals.prompts, 2);
}

#[test]
fn an_unknown_or_invalid_project_is_not_found() {
    let unknown = gitprompt_insights::stories::project_id("/srv/nothing");
    for id in [unknown, "not base64!".to_string()] {
        assert_eq!(not_found(collect(&planted(), &Request::Project { id }, false, NOW)).as_deref(), Some("project not found"));
    }
}

// --- the overview --------------------------------------------------------------------------------

#[test]
fn the_overview_is_collected_with_totals_and_every_grouping() {
    let filter = PromptFilter { before: Some(99), ..PromptFilter::default() };
    let Ok(ExportInput::Overview(overview)) = collect(&planted(), &Request::Overview { filter }, false, NOW) else {
        panic!("an overview export")
    };
    assert_eq!((overview.totals.prompts, overview.filters.before), (3, None));
    let days: Vec<(Option<&str>, i64)> = overview.groups.day.iter().map(|b| (b.key.as_deref(), b.prompts)).collect();
    assert_eq!(days, [(Some("2026-03-02"), 3)]);
    assert_eq!(overview.groups.repo.iter().map(|b| b.key.as_deref()).collect::<Vec<_>>(), [Some(REPO)]);
}

#[test]
fn an_overview_from_the_store_names_projects_relative_to_their_folder() {
    let request = Request::Overview { filter: PromptFilter::default() };
    let export = build_from_store(&planted(), &request, &Options::default(), &context(), NOW).unwrap();
    let md = render_text(&export, Format::Markdown, None).unwrap();
    assert!(md.contains("| shop | 3 | 1 | $0.75 | 0 |"), "{md}");
    assert_absent(&md, &[HOME.to_string()]);
}

// --- a prompt selection --------------------------------------------------------------------------

#[test]
fn a_prompt_selection_is_the_newest_prompts_with_their_facts_and_total() {
    let filter = PromptFilter { origin: vec![gitprompt_core::spec::Origin::Typed], ..PromptFilter::default() };
    let Ok(ExportInput::Prompts(selection)) = collect(&planted(), &Request::Prompts { filter }, false, NOW) else {
        panic!("a prompt selection")
    };
    let ids: Vec<Option<&str>> = selection.prompts.iter().map(|p| p.prompt.row.prompt_id.as_deref()).collect();
    assert_eq!((ids, selection.total), (vec![Some("p3"), Some("p1")], 2));
    assert!(selection.prompts.iter().all(|p| p.facts.is_some() && p.raw.is_none()));
}

#[test]
fn stored_payloads_are_collected_only_when_asked_for() {
    let request = Request::Prompts { filter: PromptFilter::default() };
    let Ok(ExportInput::Prompts(selection)) = collect(&planted(), &request, true, NOW) else { panic!("a prompt selection") };
    assert!(selection.prompts.iter().all(|p| p.raw.as_ref().is_some_and(|raw| raw["hook_event_name"] == "UserPromptSubmit")));
}

#[test]
fn a_prompt_selection_reads_payloads_only_when_the_options_keep_them() {
    let request = Request::Prompts { filter: PromptFilter::default() };
    let private = build_from_store(&planted(), &request, &Options::default(), &context(), NOW).unwrap();
    let open = Options { drop_raw: false, ..Options::default() };
    let with_raw = build_from_store(&planted(), &request, &open, &context(), NOW).unwrap();
    assert!(private.rows.iter().all(|r| !r.contains_key("raw")));
    assert!(with_raw.rows.iter().all(|r| r.contains_key("raw")));
    assert_absent(&render_text(&with_raw, Format::Json, None).unwrap(), &never_ever());
}

#[test]
fn without_specs_a_prompts_tool_is_named_by_its_source() {
    let request = Request::Prompts { filter: PromptFilter::default() };
    let export = build_from_store(&planted(), &request, &Options::default(), &context(), NOW).unwrap();
    assert_eq!(export.rows[0]["tool"], json!("claude-code"));
    let md = render_text(&export, Format::Markdown, None).unwrap();
    assert!(md.contains("| shop | claude-code |"), "{md}");
}
