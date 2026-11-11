//! Prompt stories (stories.rs): phases, retries, commit attribution, session and project stories and
//! a prompt's versions, on synthetic rows. Port of `tests/insights/test_stories.py` plus the phase,
//! grace and rounding boundaries and the story branches it left implicit.
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

use std::collections::BTreeMap;

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use serde_json::json;

use common::{at, commit, commit_with_stat, ids, some_facts, story, surface_ref};
use gitprompt_core::git::Commit;
use gitprompt_core::spec::{Origin, Pattern, ToolKind};
use gitprompt_insights::facts::FactRow;
use gitprompt_insights::lineage::{Change, Op};
use gitprompt_insights::stories::{
    self, BRANCH_RUN, CHAPTER_CHARS, Chapter, Diffstat, EXCERPT_CHARS, GAP_S, MAX_FILES, MAX_VERSIONS, Marker, MarkerKind, Phase,
    PhaseReason, Place, ProjectRef, RETRY_WINDOW, SessionItem, SessionStory, StoryItem, TurnTools, attribute_commits, diffstat,
    group_markers, log_window, phases, project_id, project_of, project_repo, prompt_lineage, retries,
};
use gitprompt_store::Outcome;
use gitprompt_store::timeline::TurnEvent;

fn reasons(rows: &[FactRow], found: &[Phase]) -> Vec<(PhaseReason, Vec<i64>)> {
    found.iter().map(|p| (p.reason, ids(&rows[p.range.clone()]))).collect()
}

fn default_phases(rows: &[FactRow]) -> Vec<(PhaseReason, Vec<i64>)> {
    reasons(rows, &phases(rows, GAP_S, BRANCH_RUN))
}

fn owned_hashes(owned: BTreeMap<i64, Vec<Commit>>) -> BTreeMap<i64, Vec<String>> {
    owned.into_iter().map(|(id, commits)| (id, commits.into_iter().map(|c| c.hash).collect())).collect()
}

fn turn_event(prompt_id: &str, name: &str, minutes: f64, tool: Option<&str>, input: Option<serde_json::Value>) -> TurnEvent {
    TurnEvent {
        prompt_id: prompt_id.into(),
        event: name.into(),
        received_at: at(minutes),
        tool: tool.map(Into::into),
        tool_input: input,
        description: None,
        subagent_type: None,
        prompt: None,
    }
}

fn kinds_of(entries: &[(&str, ToolKind)]) -> IndexMap<String, ToolKind> {
    entries.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

struct SessionInputs {
    tool_kinds: BTreeMap<i64, BTreeMap<ToolKind, i64>>,
    events: Vec<TurnEvent>,
    kinds: IndexMap<String, ToolKind>,
    tool_files: IndexMap<String, Pattern>,
}

impl SessionInputs {
    fn none() -> Self {
        SessionInputs { tool_kinds: BTreeMap::new(), events: vec![], kinds: IndexMap::new(), tool_files: IndexMap::new() }
    }
    fn story(&self, rows: &[FactRow], commits: &[Commit]) -> SessionStory {
        let turns = TurnTools { tool_kinds: &self.tool_kinds, events: &self.events, kinds: &self.kinds, tool_files: &self.tool_files };
        stories::session_story("s1", rows, turns, commits)
    }
}

fn chapters(story: &SessionStory) -> Vec<&Chapter> {
    story.items.iter().filter_map(|i| if let StoryItem::Chapter(c) = i { Some(c.as_ref()) } else { None }).collect()
}

fn marker(kind: MarkerKind, id: i64) -> Marker {
    Marker { kind, id, received_at: at(0.0), label: None, detail: None, excerpt: None, clipped: false }
}

fn chapter(id: i64) -> Chapter {
    Chapter {
        id,
        turn: id as usize,
        received_at: at(0.0),
        origin: "typed".into(),
        text: "x".into(),
        clipped: false,
        slash_command: None,
        branch: None,
        outcome: Outcome::Done,
        response_s: None,
        cost_usd: 0.0,
        tokens: 0,
        tool_calls: 0,
        tool_failures: 0,
        tools: BTreeMap::new(),
        files_written: 0,
        files: vec![],
        subagents: 0,
        git_commits: 0,
        commits: vec![],
        diffstat: None,
        retry_of: None,
        facts: None,
    }
}

fn item_shape(items: &[StoryItem]) -> Vec<(&'static str, usize)> {
    items
        .iter()
        .map(|i| match i {
            StoryItem::Chapter(_) => ("chapter", 0),
            StoryItem::Markers { items } => ("markers", items.len()),
        })
        .collect()
}

// --- project ids -------------------------------------------------------------------------------------

#[test]
fn project_ids_round_trip_and_are_url_safe() {
    let repos = ["/Users/me/code/app", "/tmp/a b/ü", "C:\\work\\x?y"];
    let keys = repos.map(project_id);
    assert_eq!(keys.clone().map(|k| project_repo(&k)), repos.map(|r| Some(r.to_string())));
    assert!(keys.iter().all(|k| k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')), "{keys:?}");
}

#[test]
fn a_project_id_is_unpadded_base64url_of_the_path() {
    assert_eq!(project_id("/r/app"), "L3IvYXBw");
}

#[test]
fn keys_that_are_not_project_ids_name_no_project() {
    assert_eq!([project_repo("%%%"), project_repo("_w"), project_repo("")], [None, None, None]);
}

#[test]
fn a_project_is_named_by_its_remote_else_by_its_folder() {
    let named = [story(1, 0.0, "x").build()];
    let unnamed = [story(1, 0.0, "x")
        .with(|r| {
            r.repo = Some("/r/shop/".into());
            r.repo_name = None;
            r.branch = None;
        })
        .build()];
    assert_eq!(
        project_of(&named),
        Some(ProjectRef { id: project_id("/r/app"), repo: "/r/app".into(), name: "acme/app".into(), git: true })
    );
    assert_eq!(
        project_of(&unnamed),
        Some(ProjectRef { id: project_id("/r/shop/"), repo: "/r/shop/".into(), name: "shop".into(), git: false })
    );
}

#[test]
fn rows_without_a_repo_have_no_project() {
    assert_eq!(project_of(&[story(1, 0.0, "x").with(|r| r.repo = None).build()]), None);
    assert_eq!(project_of(&[]), None);
}

// --- phases --------------------------------------------------------------------------------------------

#[test]
fn a_pause_of_three_hours_after_a_turn_ended_starts_a_phase() {
    let rows = [story(1, 0.0, "a").build(), story(2, 10.0, "b").build(), story(3, 191.0, "c").build(), story(4, 370.0, "d").build()];
    let found = phases(&rows, GAP_S, BRANCH_RUN);
    assert_eq!(reasons(&rows, &found), vec![(PhaseReason::Start, vec![1, 2]), (PhaseReason::Gap, vec![3, 4])]);
    assert_eq!((found[0].gap_s, found[1].gap_s), (None, Some(10_800)));
}

#[test]
fn a_pause_one_second_short_of_three_hours_keeps_the_phase() {
    let rows = [story(1, 10.0, "a").build(), story(2, 191.0 - 1.0 / 60.0, "b").build()];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1, 2])]);
}

#[test]
fn a_turn_without_a_response_time_ends_when_it_was_sent() {
    let rows = [story(1, 0.0, "a").response_s(None).build(), story(2, 180.0, "b").build()];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1]), (PhaseReason::Gap, vec![2])]);
}

#[test]
fn the_pause_is_rounded_half_to_even_seconds() {
    let rows = [story(1, 0.0, "a").build(), story(2, 181.0 + 0.5 / 60.0, "b").build()];
    assert_eq!(phases(&rows, GAP_S, BRANCH_RUN)[1].gap_s, Some(10_800));
}

#[test]
fn a_branch_starts_a_phase_once_it_holds() {
    let rows = [
        story(1, 0.0, "a").build(),
        story(2, 1.0, "b").branch(Some("feat")).build(),
        story(3, 2.0, "c").build(),
        story(4, 3.0, "d").branch(Some("feat")).build(),
        story(5, 4.0, "e").branch(Some("feat")).build(),
    ];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1, 2, 3]), (PhaseReason::Branch, vec![4, 5])]);
}

#[test]
fn prompts_without_a_branch_keep_the_phase() {
    let rows = [
        story(1, 0.0, "a").branch(None).build(),
        story(2, 1.0, "b").build(),
        story(3, 2.0, "c").branch(None).build(),
        story(4, 3.0, "d").branch(Some("x")).build(),
        story(5, 4.0, "e").branch(Some("x")).build(),
    ];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1, 2, 3]), (PhaseReason::Branch, vec![4, 5])]);
}

#[test]
fn a_new_branch_on_the_last_prompt_cannot_hold_yet() {
    let rows = [story(1, 0.0, "a").build(), story(2, 1.0, "b").build(), story(3, 2.0, "c").branch(Some("feat")).build()];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1, 2, 3])]);
}

#[test]
fn a_pause_wins_over_a_branch_change() {
    let rows =
        [story(1, 0.0, "a").build(), story(2, 300.0, "b").branch(Some("feat")).build(), story(3, 301.0, "c").branch(Some("feat")).build()];
    assert_eq!(default_phases(&rows), vec![(PhaseReason::Start, vec![1]), (PhaseReason::Gap, vec![2, 3])]);
}

#[test]
fn the_gap_and_the_branch_run_are_parameters() {
    let rows = [story(1, 0.0, "a").build(), story(2, 2.0, "b").branch(Some("feat")).build(), story(3, 4.0, "c").build()];
    assert_eq!(
        reasons(&rows, &phases(&rows, 3600.0, 1)),
        vec![(PhaseReason::Start, vec![1]), (PhaseReason::Branch, vec![2]), (PhaseReason::Branch, vec![3])]
    );
    assert_eq!(
        reasons(&rows, &phases(&rows, 30.0, BRANCH_RUN)),
        vec![(PhaseReason::Start, vec![1]), (PhaseReason::Gap, vec![2]), (PhaseReason::Gap, vec![3])]
    );
}

#[test]
fn no_prompts_have_no_phases() {
    assert_eq!(phases(&[], GAP_S, BRANCH_RUN), vec![]);
}

// --- retries -------------------------------------------------------------------------------------------

#[test]
fn a_retry_rewords_an_earlier_prompt_of_the_same_session() {
    let rows = [
        story(1, 0.0, "add tests for the parser").build(),
        story(2, 1.0, "explain the cache").build(),
        story(3, 2.0, "add unit tests for the parser module").build(),
        story(4, 3.0, "add tests for the parser").session(Some("s2")).build(),
    ];
    assert_eq!(retries(&rows, RETRY_WINDOW), BTreeMap::from([(3, 1)]));
}

#[test]
fn a_retry_points_to_the_latest_earlier_version() {
    let rows = [
        story(1, 0.0, "fix the login bug").build(),
        story(2, 1.0, "fix the login bug!").build(),
        story(3, 2.0, "Fix the login bug").build(),
    ];
    assert_eq!(retries(&rows, RETRY_WINDOW), BTreeMap::from([(2, 1), (3, 2)]));
}

#[test]
fn only_prompts_within_the_window_are_compared() {
    let rows = [
        story(1, 0.0, "add tests for the parser").build(),
        story(2, 1.0, "explain the cache").build(),
        story(3, 2.0, "add unit tests for the parser module").build(),
    ];
    assert_eq!((retries(&rows, 1), retries(&rows, 2)), (BTreeMap::new(), BTreeMap::from([(3, 1)])));
}

#[test]
fn prompts_without_a_session_are_compared_with_each_other() {
    let rows = [story(1, 0.0, "fix the login bug").session(None).build(), story(2, 1.0, "fix the login bug!").session(None).build()];
    assert_eq!(retries(&rows, RETRY_WINDOW), BTreeMap::from([(2, 1)]));
}

// --- commits -------------------------------------------------------------------------------------------

#[test]
fn a_commit_belongs_to_the_turn_that_made_it() {
    let rows = [
        story(1, 0.0, "a")
            .session(Some("a"))
            .with(|r| {
                r.git_commits = 1;
                r.response_s = Some(600.0);
            })
            .build(),
        story(2, 5.0, "b").session(Some("b")).build(),
        story(3, 30.0, "c").session(Some("b")).build(),
    ];
    let commits = [commit("c0", &at(-5.0)), commit("c1", &at(9.0)), commit("c2", &at(20.0)), commit("c3", &at(31.0))];
    assert_eq!(
        owned_hashes(attribute_commits(&rows, &commits)),
        BTreeMap::from([(1, vec!["c1".into()]), (2, vec!["c2".into()]), (3, vec!["c3".into()])])
    );
}

#[test]
fn a_commit_two_minutes_after_a_committing_turn_ended_still_belongs_to_it() {
    let rows = [
        story(1, 0.0, "a")
            .with(|r| {
                r.git_commits = 1;
                r.response_s = Some(600.0);
            })
            .build(),
        story(2, 5.0, "b").build(),
    ];
    let commits = [commit("in", &at(12.0)), commit("out", &at(12.0 + 1.0 / 60.0))];
    assert_eq!(owned_hashes(attribute_commits(&rows, &commits)), BTreeMap::from([(1, vec!["in".into()]), (2, vec!["out".into()])]));
}

#[test]
fn a_commit_made_as_a_prompt_was_sent_belongs_to_that_prompt() {
    let rows = [story(1, 0.0, "a").build(), story(2, 5.0, "b").build()];
    assert_eq!(owned_hashes(attribute_commits(&rows, &[commit("c", &at(5.0))])), BTreeMap::from([(2, vec!["c".into()])]));
}

#[test]
fn commits_with_an_unreadable_time_belong_to_no_prompt() {
    let rows = [story(1, 0.0, "a").build()];
    assert_eq!(attribute_commits(&rows, &[commit("bad", "not a time"), commit("empty", "")]), BTreeMap::new());
}

#[test]
fn prompts_are_ordered_by_time_before_commits_are_attributed() {
    let rows = [story(2, 5.0, "b").build(), story(1, 0.0, "a").build()];
    assert_eq!(owned_hashes(attribute_commits(&rows, &[commit("c", &at(6.0))])), BTreeMap::from([(2, vec!["c".into()])]));
}

#[test]
fn a_diffstat_sums_commits_files_insertions_and_deletions() {
    let commits = [commit_with_stat("a", &at(0.0), 1, 3, 1), commit_with_stat("b", &at(1.0), 2, 10, 0)];
    assert_eq!(diffstat(&commits), Some(Diffstat { commits: 2, files: 3, insertions: 13, deletions: 1 }));
    assert_eq!(diffstat(&[]), None);
}

#[test]
fn the_log_window_runs_from_the_first_prompt_to_two_hours_after_the_last_turn_ended() {
    let rows = [story(1, 0.0, "a").response_s(Some(3600.0)).build(), story(2, 10.0, "b").build()];
    assert_eq!(log_window(&rows), Some((at(0.0), "2026-03-02T12:00:00+00:00".to_string())));
    assert_eq!(log_window(&[]), None);
}

// --- a session -----------------------------------------------------------------------------------------

#[test]
fn markers_between_chapters_fold_into_groups() {
    let items = vec![
        SessionItem::Chapter(Box::new(chapter(1))),
        SessionItem::Marker(marker(MarkerKind::TaskNotification, 2)),
        SessionItem::Marker(marker(MarkerKind::Compaction, 2)),
        SessionItem::Chapter(Box::new(chapter(3))),
        SessionItem::Marker(marker(MarkerKind::AgentMessage, 4)),
    ];
    assert_eq!(item_shape(&group_markers(items)), vec![("chapter", 0), ("markers", 2), ("chapter", 0), ("markers", 1)]);
}

#[test]
fn markers_before_the_first_chapter_form_a_group() {
    let items = vec![SessionItem::Marker(marker(MarkerKind::Programmatic, 1)), SessionItem::Chapter(Box::new(chapter(2)))];
    assert_eq!(
        group_markers(items),
        vec![StoryItem::Markers { items: vec![marker(MarkerKind::Programmatic, 1)] }, StoryItem::Chapter(Box::new(chapter(2)))]
    );
}

#[test]
fn every_system_origin_has_its_marker_kind() {
    use MarkerKind::*;
    let found = [Origin::TaskNotification, Origin::AgentMessage, Origin::Subagent, Origin::Programmatic, Origin::Unknown]
        .map(MarkerKind::of_origin);
    assert_eq!(found, [TaskNotification, AgentMessage, Subagent, Programmatic, Unknown]);
}

fn flaky_session() -> (Vec<FactRow>, SessionInputs, Vec<Commit>) {
    let rows = vec![
        story(1, 0.0, "fix the flaky test").build(),
        story(2, 3.0, "<task-notification>build done</task-notification>").origin("task_notification").build(),
        story(3, 4.0, "fix the flaky tests please").outcome(Outcome::Failed).with(|r| r.ended_by = Some("StopFailure".into())).build(),
    ];
    let subagent = TurnEvent {
        description: Some("Look around".into()),
        subagent_type: Some("Explore".into()),
        prompt: Some("Find the test helpers".into()),
        ..turn_event("p1", "PostToolUse", 2.0, Some("Agent"), Some(json!({"prompt": "Find the test helpers"})))
    };
    let inputs = SessionInputs {
        tool_kinds: BTreeMap::from([(1, BTreeMap::from([(ToolKind::Write, 1), (ToolKind::Agent, 1)]))]),
        events: vec![
            turn_event("p1", "PostToolUse", 1.0, Some("Edit"), Some(json!({"file_path": "/r/app/a.py"}))),
            subagent,
            turn_event("p3", "PreCompact", 5.0, None, None),
        ],
        kinds: kinds_of(&[("Edit", ToolKind::Write), ("Agent", ToolKind::Agent)]),
        tool_files: IndexMap::new(),
    };
    (rows, inputs, vec![commit_with_stat("abc", &at(2.0), 1, 3, 1)])
}

#[test]
fn a_session_story_never_shows_system_text_as_typed() {
    let (rows, inputs, commits) = flaky_session();
    let story = inputs.story(&rows, &commits);
    assert_eq!(item_shape(&story.items), vec![("chapter", 0), ("markers", 2), ("chapter", 0), ("markers", 1)]);
    let [StoryItem::Chapter(first), StoryItem::Markers { items: system }, StoryItem::Chapter(retry), StoryItem::Markers { items: tail }] =
        &story.items[..]
    else {
        panic!("unexpected items {:?}", story.items);
    };
    assert_eq!((first.text.as_str(), first.files.clone()), ("fix the flaky test", vec!["/r/app/a.py".to_string()]));
    assert_eq!(first.tools, BTreeMap::from([(ToolKind::Write, 1), (ToolKind::Agent, 1)]));
    assert_eq!(first.diffstat, Some(Diffstat { commits: 1, files: 1, insertions: 3, deletions: 1 }));
    assert_eq!(
        system.iter().map(|m| (m.kind, m.id)).collect::<Vec<_>>(),
        vec![(MarkerKind::SubagentPrompt, 1), (MarkerKind::TaskNotification, 2)]
    );
    assert_eq!(
        system[0],
        Marker {
            kind: MarkerKind::SubagentPrompt,
            id: 1,
            received_at: at(2.0),
            label: Some("Look around".into()),
            detail: Some("Explore".into()),
            excerpt: Some("Find the test helpers".into()),
            clipped: false
        }
    );
    assert_eq!((retry.retry_of, retry.outcome, retry.turn), (Some(1), Outcome::Failed, 3));
    assert_eq!(tail[0].kind, MarkerKind::Compaction);
}

#[test]
fn a_session_story_totals_chapters_markers_cost_and_commits() {
    let (rows, inputs, commits) = flaky_session();
    let totals = inputs.story(&rows, &commits).totals;
    assert_eq!((totals.chapters, totals.markers, totals.cost_usd, totals.tool_calls, totals.files_written), (2, 3, 1.5, 6, 1));
    assert_eq!((totals.outcomes, totals.retries), (IndexMap::from([(Outcome::Done, 1), (Outcome::Failed, 1)]), 1));
    assert_eq!(totals.diffstat, Some(Diffstat { commits: 1, files: 1, insertions: 3, deletions: 1 }));
}

#[test]
fn a_session_story_names_its_project_title_branches_and_time_span() {
    let rows = [
        story(1, 0.0, "a").facts(some_facts("claude-code", Some("opus"), None)).build(),
        story(2, 3.0, "b")
            .with(|r| {
                r.title = Some("Fix tests".into());
                r.branch = Some("feat".into());
            })
            .facts(some_facts("claude-code", Some("opus"), Some(surface_ref("vscode", "VS Code", "code"))))
            .build(),
        story(3, 4.0, "c").facts(some_facts("claude-code", Some("haiku"), None)).build(),
    ];
    let story = SessionInputs::none().story(&rows, &[]);
    assert_eq!(story.project, Some(ProjectRef { id: project_id("/r/app"), repo: "/r/app".into(), name: "acme/app".into(), git: true }));
    assert_eq!(
        (story.title.as_deref(), story.source.as_deref(), story.branches),
        (Some("Fix tests"), Some("claude-code"), vec!["main".to_string(), "feat".to_string()])
    );
    assert_eq!((story.started_at, story.ended_at), (Some(at(0.0)), Some("2026-03-02T09:05:00+00:00".to_string())));
    assert_eq!(
        (story.connector.map(|c| c.id), story.surface),
        (Some("claude-code".to_string()), Some(surface_ref("vscode", "VS Code", "code")))
    );
    assert_eq!(story.models, IndexMap::from([("opus".to_string(), 2), ("haiku".to_string(), 1)]));
}

#[test]
fn an_empty_session_story_has_no_items_and_no_span() {
    let story = SessionInputs::none().story(&[], &[]);
    assert_eq!((story.items, story.started_at, story.ended_at, story.project, story.connector), (vec![], None, None, None, None));
    assert_eq!((story.totals.chapters, story.totals.diffstat), (0, None));
}

#[test]
fn a_chapter_reports_tokens_cost_and_the_command() {
    let rows = [story(1, 0.0, "/review now").origin("slash_command").with(|r| r.slash_command = Some("/review".into())).build()];
    let story = SessionInputs::none().story(&rows, &[]);
    let chapter = chapters(&story)[0];
    assert_eq!(
        (chapter.tokens, chapter.cost_usd, chapter.slash_command.as_deref(), chapter.origin.as_str()),
        (15, 0.5, Some("/review"), "slash_command")
    );
}

#[test]
fn long_chapter_texts_are_clipped() {
    let story = SessionInputs::none().story(&[story(1, 0.0, &"x".repeat(CHAPTER_CHARS + 10)).build()], &[]);
    let chapter = chapters(&story)[0];
    assert_eq!((chapter.clipped, chapter.text.chars().count(), chapter.text.ends_with('…')), (true, CHAPTER_CHARS + 1, true));
}

#[test]
fn a_marker_shows_a_clipped_excerpt_of_the_system_text() {
    let rows =
        [story(1, 0.0, &"y".repeat(EXCERPT_CHARS + 20)).origin("agent_message").build(), story(2, 1.0, "").origin("programmatic").build()];
    let story = SessionInputs::none().story(&rows, &[]);
    let [StoryItem::Markers { items }] = &story.items[..] else { panic!("{:?}", story.items) };
    assert_eq!((items[0].clipped, items[0].excerpt.as_ref().map(|e| e.chars().count())), (true, Some(EXCERPT_CHARS + 1)));
    assert_eq!((items[1].kind, items[1].excerpt.clone(), items[1].clipped), (MarkerKind::Programmatic, None, false));
}

#[test]
fn a_chapter_lists_at_most_twenty_sorted_files() {
    let mut inputs = SessionInputs::none();
    inputs.kinds = kinds_of(&[("Edit", ToolKind::Write)]);
    inputs.events = (0..25)
        .rev()
        .map(|n| turn_event("p1", "PostToolUse", 1.0, Some("Edit"), Some(json!({"file_path": format!("/f{n:02}")}))))
        .collect();
    let story = inputs.story(&[story(1, 0.0, "edit everything").build()], &[]);
    let files = chapters(&story)[0].files.clone();
    assert_eq!(
        (files.len(), files.first().cloned(), files.last().cloned()),
        (MAX_FILES, Some("/f00".to_string()), Some("/f19".to_string()))
    );
    assert_eq!(story.totals.files_written, MAX_FILES as u64);
}

#[test]
fn files_named_inside_a_patch_count_for_the_chapter() {
    let mut inputs = SessionInputs::none();
    inputs.kinds = kinds_of(&[("apply_patch", ToolKind::Write)]);
    inputs.tool_files =
        IndexMap::from([("apply_patch".to_string(), Pattern::new(r"(?m)^\*\*\* (?:Add|Update|Delete) File: (.+)$").unwrap())]);
    inputs.events = vec![turn_event(
        "p1",
        "PostToolUse",
        1.0,
        Some("apply_patch"),
        Some(json!({"command": "*** Begin Patch\n*** Update File: /work/app/upload.py\n*** End Patch"})),
    )];
    let story = inputs.story(&[story(1, 0.0, "add a retry").build()], &[]);
    assert_eq!(chapters(&story)[0].files, vec!["/work/app/upload.py".to_string()]);
}

#[test]
fn reading_tools_and_agent_calls_without_a_brief_add_nothing() {
    let mut inputs = SessionInputs::none();
    inputs.kinds = kinds_of(&[("Read", ToolKind::Read), ("Agent", ToolKind::Agent)]);
    inputs.events = vec![
        turn_event("p1", "PostToolUse", 1.0, Some("Read"), Some(json!({"file_path": "/a"}))),
        turn_event("p1", "PostToolUse", 1.0, Some("Agent"), Some(json!({}))),
    ];
    let story = inputs.story(&[story(1, 0.0, "look").build()], &[]);
    assert_eq!((item_shape(&story.items), chapters(&story)[0].files.clone()), (vec![("chapter", 0)], vec![]));
}

#[test]
fn commits_go_to_chapters_never_to_system_prompts() {
    let rows = [story(1, 0.0, "fix it").build(), story(2, 5.0, "<agent-message>hi</agent-message>").origin("agent_message").build()];
    let story = SessionInputs::none().story(&rows, &[commit("c", &at(6.0))]);
    assert_eq!(chapters(&story)[0].commits.iter().map(|c| c.hash.as_str()).collect::<Vec<_>>(), vec!["c"]);
}

#[test]
fn a_session_story_serializes_items_with_their_kind() {
    let story = SessionInputs::none().story(&[story(1, 0.0, "a").build(), story(2, 1.0, "b").origin("subagent").build()], &[]);
    let value = serde_json::to_value(&story.items).unwrap();
    assert_eq!(
        (value[0]["kind"].clone(), value[1]["kind"].clone(), value[1]["items"][0]["kind"].clone()),
        (json!("chapter"), json!("markers"), json!("subagent"))
    );
}

// --- a project -----------------------------------------------------------------------------------------

#[test]
fn a_project_story_has_phases_with_measures_commits_and_retries() {
    let rows = [
        story(1, 0.0, "set up the project").with(|r| r.mentions = 1).build(),
        story(2, 5.0, "add the parser")
            .outcome(Outcome::Superseded)
            .with(|r| {
                r.ended_by = None;
                r.response_s = None;
            })
            .build(),
        story(3, 6.0, "add the parser module").build(),
        story(4, 60.0 * 24.0, "write the docs").session(Some("s2")).with(|r| r.code_blocks = 1).build(),
    ];
    let commits = [commit_with_stat("c1", &at(8.0), 2, 10, 0)];
    let project = stories::project_story("/r/app", &rows, &BTreeMap::new(), &commits, 7);
    assert_eq!((project.project.id.clone(), project.system), (project_id("/r/app"), 7));
    let shape: Vec<(PhaseReason, Vec<i64>)> = project.phases.iter().map(|p| (p.reason, p.items.iter().map(|e| e.id).collect())).collect();
    assert_eq!(shape, vec![(PhaseReason::Start, vec![1, 2, 3]), (PhaseReason::Gap, vec![4])]);
    let first = &project.phases[0];
    assert_eq!((first.index, first.measures.outcomes.clone()), (1, IndexMap::from([(Outcome::Done, 2), (Outcome::Superseded, 1)])));
    assert_eq!(
        (first.measures.retries, first.measures.with_mention, first.measures.diffstat),
        (1, 1, Some(Diffstat { commits: 1, files: 2, insertions: 10, deletions: 0 }))
    );
    assert_eq!((first.items[2].commits[0].hash.as_str(), first.items[2].retry_of), ("c1", Some(2)));
    assert_eq!((project.totals.prompts, project.totals.sessions, project.totals.with_code), (4, 2, 1));
}

#[test]
fn a_project_story_keeps_only_what_you_typed() {
    let rows = [story(1, 0.0, "x").build(), story(2, 1.0, "<agent-message>hi</agent-message>").origin("agent_message").build()];
    let project = stories::project_story("/r/app", &rows, &BTreeMap::new(), &[], 0);
    assert_eq!(project.phases.iter().flat_map(|p| p.items.iter().map(|e| e.id)).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn the_median_prompt_size_rounds_half_to_even() {
    let low = [story(1, 0.0, "ab").with(|r| r.chars = 2).build(), story(2, 1.0, "abc").with(|r| r.chars = 3).build()];
    let high = [story(1, 0.0, "abc").with(|r| r.chars = 3).build(), story(2, 1.0, "abcd").with(|r| r.chars = 4).build()];
    let median = |rows: &[FactRow]| stories::project_story("/r/app", rows, &BTreeMap::new(), &[], 0).totals.median_chars;
    assert_eq!((median(&low), median(&high)), (2, 4));
}

#[test]
fn a_phase_lists_its_branches_span_and_distinct_sessions() {
    let rows = [
        story(1, 0.0, "a").build(),
        story(2, 1.0, "b").branch(Some("feat")).session(None).build(),
        story(3, 2.0, "c").session(Some("s2")).build(),
    ];
    let phase = &stories::project_story("/r/app", &rows, &BTreeMap::new(), &[], 0).phases[0];
    assert_eq!((phase.branches.clone(), phase.measures.sessions), (vec!["main".to_string(), "feat".to_string()], 2));
    assert_eq!(
        (phase.measures.first_at.clone(), phase.measures.last_at.clone(), phase.measures.median_chars),
        (Some(at(0.0)), Some(at(2.0)), 1)
    );
}

#[test]
fn a_project_without_typed_prompts_is_named_by_its_system_prompts() {
    let rows = [story(1, 0.0, "<agent-message>hi</agent-message>").origin("agent_message").build()];
    let project = stories::project_story("/r/app", &rows, &BTreeMap::new(), &[], 1);
    assert_eq!(
        (project.project.name.as_str(), project.phases.len(), project.totals.prompts, project.totals.median_chars),
        ("acme/app", 0, 0, 0)
    );
}

#[test]
fn a_project_without_rows_is_named_by_its_folder_path() {
    let project = stories::project_story("/r/empty", &[], &BTreeMap::new(), &[], 0);
    assert_eq!(project.project, ProjectRef { id: project_id("/r/empty"), repo: "/r/empty".into(), name: "/r/empty".into(), git: false });
    assert_eq!((project.totals.first_at, project.totals.diffstat), (None, None));
}

// --- a prompt's lineage ----------------------------------------------------------------------------------

#[test]
fn a_lineage_orders_versions_and_says_what_changed() {
    let target = story(3, 10.0, "add unit tests for the parser module").build();
    let candidates = [
        story(1, 0.0, "add tests for the parser").outcome(Outcome::Failed).build(),
        story(2, 5.0, "add tests for the parser")
            .session(Some("s9"))
            .with(|r| {
                r.repo = Some("/r/other".into());
                r.repo_name = None;
            })
            .build(),
        story(4, 20.0, "explain the cache").build(),
        story(5, 30.0, "add tests for the parser").origin("task_notification").build(),
        target.clone(),
    ];
    let found = prompt_lineage(&target, &candidates);
    let versions = &found.versions;
    assert_eq!(
        versions.iter().map(|v| (v.id, v.place)).collect::<Vec<_>>(),
        vec![(1, Place::Session), (2, Place::Elsewhere), (3, Place::Itself)]
    );
    assert_eq!(
        (versions[0].diff.clone(), versions[0].change, versions[1].change, versions[2].change),
        (None, None, Some(Change::Improved), Some(Change::Same))
    );
    assert!(
        versions[2].diff.as_ref().is_some_and(|d| d.iter().any(|(op, t)| *op == Op::Insert && t.contains("unit"))),
        "{:?}",
        versions[2].diff
    );
    assert_eq!(
        (versions[1].project.as_ref().map(|p| p.name.clone()), versions[2].similarity, found.threshold),
        (Some("other".to_string()), 1.0, 0.5)
    );
}

#[test]
fn another_session_of_the_same_repo_is_the_project_and_equal_texts_are_marked() {
    let target = story(2, 10.0, "add tests for the parser").build();
    let candidates = [story(1, 0.0, "Add tests for the parser")
        .session(Some("s2"))
        .with(|r| r.text_hash = Some("add tests for the parser".into()))
        .build()];
    let versions = prompt_lineage(&target, &candidates).versions;
    assert_eq!(
        versions.iter().map(|v| (v.id, v.place, v.same_text)).collect::<Vec<_>>(),
        vec![(1, Place::Project, true), (2, Place::Itself, true)]
    );
}

#[test]
fn candidates_whose_size_is_far_off_are_skipped_before_comparing() {
    let target = story(2, 10.0, "add tests for the parser").build();
    let candidates = [story(1, 0.0, "add tests for the parser").with(|r| r.chars = 5).build()];
    assert_eq!(prompt_lineage(&target, &candidates).versions.iter().map(|v| v.id).collect::<Vec<_>>(), vec![2]);
}

#[test]
fn a_lineage_keeps_at_most_the_prompt_and_its_closest_versions() {
    let target = story(100, 100.0, "add tests for the parser").build();
    let candidates: Vec<FactRow> = (1..=25).map(|n| story(n, n as f64, "add tests for the parser!").build()).collect();
    let versions = prompt_lineage(&target, &candidates).versions;
    assert_eq!((versions.len(), versions.last().map(|v| v.id)), (MAX_VERSIONS, Some(100)));
}

#[test]
fn a_system_prompt_has_no_lineage() {
    let target = story(1, 0.0, "<task-notification>x</task-notification>").origin("task_notification").build();
    let found = prompt_lineage(&target, &[target.clone(), story(2, 1.0, "<task-notification>x</task-notification>").build()]);
    assert_eq!((found.id, found.versions.len()), (1, 0));
}
