//! `HistoryCache`: a repository's commits are listed again only when a local branch moved.
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

use std::time::Duration;

use common::{Repo, repo};
use gitprompt_core::git::{HistoryCache, history};
use pretty_assertions::assert_eq;

const SINCE: &str = "2026-01-01T00:00:00Z";
const LONG: Duration = Duration::from_secs(600);

/// A repository with one commit, `first`, on 2 January.
fn committed() -> Repo {
    let repo = repo();
    repo.commit("a.txt", "a\n", "first", "2026-01-02T10:00:00+00:00");
    repo
}

fn subjects(cache: &HistoryCache, repo: &Repo, until: &str) -> Vec<String> {
    cache.history(&repo.root, SINCE, until).into_iter().map(|commit| commit.subject).collect()
}

#[test]
fn the_list_is_what_git_gives() {
    let repo = committed();
    let until = "2026-01-03T00:00:00Z";
    assert_eq!(HistoryCache::new(LONG).history(&repo.root, SINCE, until), history(&repo.root, SINCE, until));
}

#[test]
fn a_later_end_reads_nothing_again_while_no_branch_moved() {
    let repo = committed();
    let cache = HistoryCache::new(Duration::ZERO); // every read asks git where the branches are
    assert_eq!(subjects(&cache, &repo, "2026-01-03T00:00:00Z"), ["first"]);
    assert_eq!(subjects(&cache, &repo, "2026-01-03T00:00:07.5Z"), ["first"]);
    assert_eq!(cache.misses(), 1);
}

#[test]
fn a_new_commit_is_listed_at_once() {
    let repo = committed();
    let cache = HistoryCache::new(Duration::ZERO);
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:00Z"), ["first"]);
    repo.commit("b.txt", "b\n", "second", "2026-01-04T10:00:00+00:00");
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:01Z"), ["first", "second"]);
    assert_eq!(cache.misses(), 2);
}

#[test]
fn within_the_ttl_git_is_not_asked_at_all() {
    let repo = committed();
    let cache = HistoryCache::new(LONG);
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:00Z"), ["first"]);
    repo.commit("b.txt", "b\n", "second", "2026-01-04T10:00:00+00:00"); // not seen until the ttl is over
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:01Z"), ["first"]);
    assert_eq!(cache.misses(), 1);
}

#[test]
fn an_earlier_end_is_read_from_git() {
    let repo = committed();
    repo.commit("b.txt", "b\n", "second", "2026-01-04T10:00:00+00:00");
    let cache = HistoryCache::new(LONG);
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:00Z"), ["first", "second"]);
    assert_eq!(subjects(&cache, &repo, "2026-01-03T00:00:00Z"), ["first"]);
}

#[test]
fn an_empty_list_is_never_kept() {
    let repo = repo();
    let cache = HistoryCache::new(LONG);
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:00Z"), Vec::<String>::new());
    repo.commit("a.txt", "a\n", "first", "2026-01-02T10:00:00+00:00");
    assert_eq!(subjects(&cache, &repo, "2026-01-05T00:00:00Z"), ["first"]);
}
