//! A commit by its hash (`stories/commit_lookup.rs`) and the commits of find (`find::commit_hits`):
//! both read the projects' traced commits, so a hash names the same commit everywhere.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, a remote lookup returns an Option by contract, and helpers are crate-private anyway"
)]

mod common;

use pretty_assertions::assert_eq;

use common::{at, commit, story};
use gitprompt_core::git::Commit;
use gitprompt_insights::find::{PER_KIND, Target, commit_hits};
use gitprompt_insights::stories::{CommitLookup, FoundCommit, ProjectTraces, TraceHow, lookup_commit, names_hash, project_traces};

/// A commit with its own subject.
fn titled(hash: &str, subject: &str, minutes: f64) -> Commit {
    Commit { subject: subject.into(), ..commit(hash, &at(minutes)) }
}

/// The project `/r/<name>`, one prompt at minute 0, and these commits.
fn project(name: &str, commits: &[Commit]) -> ProjectTraces {
    let repo = format!("/r/{name}");
    let rows = [story(1, 0.0, "add the uploader").with(|r| r.repo = Some(repo.clone())).build()];
    project_traces(&repo, &rows, commits)
}

fn github(_: &str) -> Option<String> {
    Some("git@github.com:acme/app.git".into())
}

fn hashes(found: &[FoundCommit]) -> Vec<&str> {
    found.iter().map(|f| f.trace.commit.hash.as_str()).collect()
}

// --- a hash ----------------------------------------------------------------------------------------

#[test]
fn a_hash_names_its_commit_either_way_round_ignoring_case() {
    assert!(names_hash("a1b2c3d", "A1B2"));
    assert!(names_hash("a1b2c3d", "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678"), "a full hash finds the short one");
    assert!(!names_hash("a1b2c3d", "b2c3"), "not from the middle");
    assert!(!names_hash("a1b2c3d", ""));
}

#[test]
fn a_full_hash_or_a_prefix_finds_the_trace_with_its_forge_url() {
    let projects = [project("app", &[titled("a1b2c3d", "Add the retry loop", 0.5)])];
    for hash in ["a1b2c3d", "a1b2", "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678"] {
        let Some(CommitLookup::One { commit }) = lookup_commit(hash, &projects, github) else { panic!("{hash}: not one") };
        assert_eq!(commit.trace.how, TraceHow::Traced);
        assert_eq!(commit.trace.prompt.as_ref().map(|p| p.id), Some(1));
        assert_eq!(commit.project.repo, "/r/app");
        assert_eq!(commit.trace.url.as_deref(), Some("https://github.com/acme/app/commit/a1b2c3d"));
    }
}

#[test]
fn an_ambiguous_prefix_gives_every_candidate_newest_first_across_projects() {
    let projects = [
        project("app", &[titled("abc1111", "one", 0.5), titled("abc2222", "two", 0.7)]),
        project("web", &[titled("abc3333", "three", 0.6)]),
    ];
    let Some(CommitLookup::Many { candidates }) = lookup_commit("abc", &projects, |_| None) else { panic!("not many") };
    assert_eq!(hashes(&candidates), ["abc2222", "abc3333", "abc1111"]);
    assert!(candidates.iter().all(|c| c.trace.url.is_none()), "no remote, no url");
}

#[test]
fn an_unknown_hash_finds_nothing() {
    let projects = [project("app", &[titled("a1b2c3d", "x", 0.5)])];
    assert_eq!(lookup_commit("ffff", &projects, github), None);
}

#[test]
fn a_remote_on_an_unknown_host_gives_no_url() {
    let projects = [project("app", &[titled("a1b2c3d", "x", 0.5)])];
    let Some(CommitLookup::One { commit }) = lookup_commit("a1b2c3d", &projects, |_| Some("git@example.com:acme/app.git".into())) else {
        panic!("not one")
    };
    assert_eq!(commit.trace.url, None);
}

// --- find --------------------------------------------------------------------------------------------

fn found(hits: &[gitprompt_insights::find::Hit]) -> Vec<&str> {
    hits.iter()
        .map(|h| match &h.target {
            Target::Commit { hash, .. } => hash.as_str(),
            other => panic!("not a commit: {other:?}"),
        })
        .collect()
}

#[test]
fn find_matches_a_commit_by_the_first_words_of_its_subject_or_seven_characters_of_its_hash() {
    let projects = [project("app", &[titled("a1b2c3d", "Add the retry loop", 0.5), titled("e5f6a7b", "Wire the uploader", 0.7)])];
    assert_eq!(found(&commit_hits(&projects, "add the", PER_KIND)), ["a1b2c3d"]);
    assert_eq!(found(&commit_hits(&projects, "e5f6a7b", PER_KIND)), ["e5f6a7b"]);
    let hit = &commit_hits(&projects, "wire", PER_KIND)[0];
    assert_eq!(hit.target, Target::Commit { project: projects[0].project.id.clone(), hash: "e5f6a7b".into() });
    assert_eq!((hit.title.as_deref(), hit.at.as_str()), (Some("acme/app"), at(0.7).as_str()));
}

#[test]
fn find_names_a_hash_only_with_one_word_of_four_characters_or_more() {
    let projects = [project("app", &[titled("add1234", "Fix the parser", 0.5)])];
    assert!(commit_hits(&projects, "add", PER_KIND).is_empty(), "three characters are a word, not a hash");
    assert!(commit_hits(&projects, "add1 parser", PER_KIND).is_empty(), "a hash is one word");
    assert_eq!(found(&commit_hits(&projects, "ADD1", PER_KIND)), ["add1234"]);
}

#[test]
fn find_without_words_gives_the_newest_commits() {
    let commits: Vec<Commit> = (0..7).map(|i| titled(&format!("c{i}00000"), "x", f64::from(i))).collect();
    let projects = [project("app", &commits)];
    assert_eq!(found(&commit_hits(&projects, "", PER_KIND)), ["c600000", "c500000", "c400000", "c300000", "c200000"]);
}
