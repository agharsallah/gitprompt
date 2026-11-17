//! Find's ranking (find.rs): text match, then outcome, then recency, at most five per kind.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

use pretty_assertions::assert_eq;

use gitprompt_insights::find::{Hit, PER_KIND, Target, project_hits, prompt_hits, session_hits, session_texts, session_title};
use gitprompt_insights::stories::project_id;
use gitprompt_store::filter::Outcome;
use gitprompt_store::index::IndexRow;
use gitprompt_store::stats::Bucket;
use gitprompt_store::timeline::IndexedPrompt;

fn prompt(id: i64, text: &str, outcome: Outcome, at: &str) -> IndexedPrompt {
    let row = IndexRow { id, typed_text: Some(text.into()), received_at: at.into(), ..IndexRow::default() };
    IndexedPrompt { row, outcome }
}

fn group(key: &str, detail: &str, last_at: &str) -> Bucket {
    Bucket { key: Some(key.into()), detail: Some(detail.into()), last_at: Some(last_at.into()), ..Bucket::default() }
}

fn prompt_ids(hits: &[Hit]) -> Vec<i64> {
    hits.iter()
        .map(|h| match &h.target {
            Target::Prompt { id } => *id,
            other => panic!("not a prompt: {other:?}"),
        })
        .collect()
}

fn session_ids(hits: &[Hit]) -> Vec<&str> {
    hits.iter()
        .map(|h| match &h.target {
            Target::Session { id } => id.as_str(),
            other => panic!("not a session: {other:?}"),
        })
        .collect()
}

#[test]
fn a_finished_prompt_ranks_before_a_failed_one_at_an_equal_match() {
    let rows = [
        prompt(1, "fix the parser", Outcome::Failed, "2026-01-03T00:00:00+00:00"),
        prompt(2, "fix the parser", Outcome::Done, "2026-01-01T00:00:00+00:00"),
        prompt(3, "fix the parser", Outcome::Open, "2026-01-02T00:00:00+00:00"),
        prompt(4, "fix the parser", Outcome::Interrupted, "2026-01-04T00:00:00+00:00"),
    ];
    assert_eq!(prompt_ids(&prompt_hits(&rows, "parser", PER_KIND)), vec![2, 3, 4, 1]);
}

#[test]
fn a_better_text_match_outranks_a_better_outcome() {
    let rows = [
        prompt(1, "fix the parser", Outcome::Done, "2026-01-02T00:00:00+00:00"),
        prompt(2, "parser", Outcome::Failed, "2026-01-01T00:00:00+00:00"),
    ];
    // exactly the query beats text that merely contains it, finished or not.
    assert_eq!(prompt_ids(&prompt_hits(&rows, "parser", PER_KIND)), vec![2, 1]);
    let words = [
        prompt(4, "lexer then parser", Outcome::Done, "2026-01-01T00:00:00+00:00"),
        prompt(5, "fix parser lexer", Outcome::Failed, "2026-01-01T00:00:00+00:00"),
    ];
    assert_eq!(prompt_ids(&prompt_hits(&words, "parser lexer", PER_KIND)), vec![5, 4], "the phrase beats the words");
}

#[test]
fn recency_breaks_ties_between_equal_matches_and_outcomes() {
    let rows = [
        prompt(1, "fix the parser", Outcome::Done, "2026-01-01T00:00:00+00:00"),
        prompt(2, "fix the parser", Outcome::Done, "2026-01-03T00:00:00+00:00"),
        prompt(3, "fix the parser", Outcome::Done, "2026-01-02T00:00:00+00:00"),
    ];
    assert_eq!(prompt_ids(&prompt_hits(&rows, "parser", PER_KIND)), vec![2, 3, 1]);
}

#[test]
fn at_most_five_hits_per_kind_whatever_the_limit() {
    let rows: Vec<IndexedPrompt> = (1..=9).map(|i| prompt(i, "parser", Outcome::Done, &format!("2026-01-0{i}T00:00:00+00:00"))).collect();
    assert_eq!(prompt_ids(&prompt_hits(&rows, "parser", 50)), vec![9, 8, 7, 6, 5]);
    assert_eq!(prompt_ids(&prompt_hits(&rows, "parser", 2)), vec![9, 8]);
    let sessions: Vec<Bucket> = (1..=9).map(|i| group(&format!("s{i}"), "parser work", &format!("2026-01-0{i}T00:00:00+00:00"))).collect();
    assert_eq!(session_hits(&sessions, "parser", 50).len(), PER_KIND);
}

#[test]
fn an_empty_query_gives_the_most_recent_whatever_the_outcome() {
    let rows = [
        prompt(1, "old and done", Outcome::Done, "2026-01-01T00:00:00+00:00"),
        prompt(2, "newest but failed", Outcome::Failed, "2026-01-03T00:00:00+00:00"),
        prompt(3, "middle", Outcome::Done, "2026-01-02T00:00:00+00:00"),
    ];
    assert_eq!(prompt_ids(&prompt_hits(&rows, "", PER_KIND)), vec![2, 3, 1]);
    let sessions = [group("a", "older", "2026-01-01T00:00:00+00:00"), group("b", "newer", "2026-01-02T00:00:00+00:00")];
    assert_eq!(session_ids(&session_hits(&sessions, "  ", PER_KIND)), vec!["b", "a"]);
}

#[test]
fn a_hit_carries_its_snippet_title_time_and_outcome() {
    let mut row = prompt(7, "please fix the Parser soon", Outcome::Done, "2026-01-01T00:00:00+00:00");
    row.row.title = Some("Parser work".into());
    let hits = prompt_hits(&[row], "parser", PER_KIND);
    let hit = &hits[0];
    assert_eq!(hit.title.as_deref(), Some("Parser work"));
    assert_eq!((hit.at.as_str(), hit.outcome), ("2026-01-01T00:00:00+00:00", Some(Outcome::Done)));
    let marked: Vec<&str> = hit.snippet.iter().filter(|s| s.hit).map(|s| s.text.as_str()).collect();
    assert_eq!(marked, vec!["Parser"]);
}

#[test]
fn sessions_match_on_their_title_text_and_others_are_left_out() {
    let sessions = [
        group("s1", "Refactor the parser", "2026-01-01T00:00:00+00:00"),
        group("s2", "Write release notes", "2026-01-02T00:00:00+00:00"),
        Bucket { key: None, detail: Some("parser without a session".into()), ..Bucket::default() },
    ];
    let hits = session_hits(&sessions, "PARSER", PER_KIND);
    assert_eq!(session_ids(&hits), vec!["s1"]);
    assert_eq!(hits[0].title.as_deref(), Some("Refactor the parser"));
    assert_eq!(hits[0].outcome, None);
}

#[test]
fn a_session_is_called_by_your_name_then_its_label_then_the_tools_title() {
    let titled = group("s", "Tool title", "");
    assert_eq!(session_title(&titled), Some("Tool title".into()));
    let labelled = Bucket { label: Some("Fix the parser".into()), ..titled };
    assert_eq!(session_title(&labelled), Some("Fix the parser".into()));
    let named = Bucket { name: Some("Parser rewrite".into()), ..labelled };
    assert_eq!(session_title(&named), Some("Parser rewrite".into()));
    assert_eq!(session_texts(&named).collect::<Vec<_>>(), ["Parser rewrite", "Fix the parser", "Tool title"]);
    assert_eq!(session_title(&Bucket::default()), None);
}

#[test]
fn a_session_matches_on_its_name_or_its_label_and_is_shown_by_its_name() {
    let named =
        Bucket { name: Some("Release prep".into()), label: Some("Fix the parser".into()), ..group("s1", "", "2026-01-01T00:00:00+00:00") };
    let plain = group("s2", "Write release notes", "2026-01-02T00:00:00+00:00");
    let sessions = [named, plain];

    let by_label = session_hits(&sessions, "parser", PER_KIND);
    assert_eq!(session_ids(&by_label), vec!["s1"]);
    assert_eq!(by_label[0].title.as_deref(), Some("Release prep"), "a hit is called by your name, whatever matched");

    let by_name = session_hits(&sessions, "release prep", PER_KIND);
    assert_eq!(session_ids(&by_name), vec!["s1"], "only the session whose name holds both words");
}

#[test]
fn projects_match_on_their_name_with_the_repo_folder_as_the_fallback_and_link_by_project_id() {
    let named = group("/work/app", "acme/app", "2026-01-01T00:00:00+00:00");
    let unnamed = Bucket { detail: None, ..group("/work/app-tools/", "", "2026-01-02T00:00:00+00:00") };
    let hits = project_hits(&[named, unnamed], "app", PER_KIND);
    assert_eq!(hits.iter().map(|h| h.title.as_deref()).collect::<Vec<_>>(), vec![Some("app-tools"), Some("acme/app")]);
    assert_eq!(hits[1].target, Target::Project { id: project_id("/work/app") });
    let none = project_hits(&[group("/work/other", "acme/other", "")], "app", PER_KIND);
    assert_eq!(none.len(), 0);
}
