//! Commit traces (`stories/commits.rs`, `stories/commit_list.rs`): each commit with the prompt behind
//! it, how sure that is, and the project's commit list searched and grouped. The property test
//! checks the one promise everything else rests on: reading the rule from the commits' side gives
//! exactly what `attribute_commits` gives from the prompts' side.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;
use proptest::prelude::*;

use common::{at, commit, story};
use gitprompt_core::git::Commit;
use gitprompt_insights::facts::FactRow;
use gitprompt_insights::stories::{
    COMMIT_GRACE_S, CommitGrouping, CommitQuery, CommitTrace, LOG_LEAD_S, ProjectCommits, TraceCounts, TraceHow, attribute_commits,
    commit_traces, project_commits, trace_window,
};

/// `(hash, prompt id, how)` of each trace.
fn read(traces: &[CommitTrace]) -> Vec<(String, Option<i64>, TraceHow)> {
    traces.iter().map(|t| (t.commit.hash.clone(), t.prompt.as_ref().map(|p| p.id), t.how)).collect()
}

/// A prompt whose turn ran `git commit` for `minutes`.
fn committing(id: i64, minutes: f64, session: &str, length: f64) -> FactRow {
    story(id, minutes, "commit it")
        .session(Some(session))
        .with(|r| {
            r.git_commits = 1;
            r.response_s = Some(length * 60.0);
        })
        .build()
}

/// The grace after a turn, in minutes.
fn grace() -> f64 {
    COMMIT_GRACE_S as f64 / 60.0
}

// --- the rule, read backwards ----------------------------------------------------------------------

#[test]
fn a_commit_made_during_a_turn_is_traced_to_its_prompt_and_session() {
    let rows = [story(1, 0.0, "add the login form").session(Some("a")).build()];
    let traces = commit_traces(&rows, &[commit("c1", &at(0.5))]);
    assert_eq!(read(&traces), [("c1".into(), Some(1), TraceHow::Traced)]);
    let prompt = traces[0].prompt.as_ref().unwrap();
    assert_eq!(
        (prompt.text.as_str(), prompt.branch.as_deref(), traces[0].session.as_deref()),
        ("add the login form", Some("main"), Some("a"))
    );
}

#[test]
fn a_commit_within_the_grace_after_a_turn_is_traced_and_one_after_it_is_likely() {
    // The one-minute turn ends at 1.0.
    let rows = [story(1, 0.0, "a").build()];
    let commits = [commit("in", &at(1.0 + grace())), commit("out", &at(1.0 + grace() + 1.0 / 60.0))];
    assert_eq!(
        read(&commit_traces(&rows, &commits)),
        [("in".into(), Some(1), TraceHow::Traced), ("out".into(), Some(1), TraceHow::Likely)]
    );
}

#[test]
fn a_commit_before_every_prompt_or_with_an_unreadable_time_is_made_by_hand() {
    let rows = [story(1, 0.0, "a").build()];
    let traces = commit_traces(&rows, &[commit("early", &at(-1.0)), commit("bad", "not a time")]);
    assert_eq!(read(&traces), [("early".into(), None, TraceHow::ByHand), ("bad".into(), None, TraceHow::ByHand)]);
    assert_eq!(traces[0].session, None);
}

#[test]
fn a_running_committing_turn_of_another_session_owns_the_commit() {
    let rows = [committing(1, 0.0, "a", 10.0), story(2, 5.0, "b").session(Some("b")).build()];
    let traces = commit_traces(&rows, &[commit("c", &at(9.0))]);
    assert_eq!((read(&traces), traces[0].session.as_deref()), (vec![("c".into(), Some(1), TraceHow::Traced)], Some("a")));
}

#[test]
fn without_prompts_every_commit_is_made_by_hand() {
    assert_eq!(read(&commit_traces(&[], &[commit("c", &at(0.0))])), [("c".into(), None, TraceHow::ByHand)]);
}

#[test]
fn the_trace_window_starts_two_hours_before_the_first_prompt() {
    let rows = [story(1, 120.0, "a").response_s(Some(3600.0)).build()];
    let since = at(120.0 - LOG_LEAD_S as f64 / 60.0);
    assert_eq!(trace_window(&rows), Some((since, "2026-03-02T14:00:00+00:00".to_string())));
    assert_eq!(trace_window(&[]), None);
}

// --- forward and backward agree ------------------------------------------------------------------

/// prompt id -> hashes, as `attribute_commits` gives them and as read back from the traces.
fn both_ways(rows: &[FactRow], commits: &[Commit]) -> (BTreeMap<i64, Vec<String>>, BTreeMap<i64, Vec<String>>) {
    let forward = attribute_commits(rows, commits).into_iter().map(|(id, cs)| (id, cs.into_iter().map(|c| c.hash).collect())).collect();
    let mut backward: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for trace in commit_traces(rows, commits) {
        if let Some(prompt) = trace.prompt {
            backward.entry(prompt.id).or_default().push(trace.commit.hash);
        }
    }
    (forward, backward)
}

/// A prompt: minute sent, turn length in seconds (none: unknown), whether it committed, session.
fn prompt_strategy() -> impl Strategy<Value = (u16, Option<u16>, bool, u8)> {
    (0..600u16, prop::option::of(0..1200u16), any::<bool>(), 0..3u8)
}

/// A commit's minute (some before every prompt, some after), or an unreadable time.
fn commit_strategy() -> impl Strategy<Value = Option<i16>> {
    prop::option::weighted(0.95, -60..800i16)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn forward_and_backward_links_agree(
        prompts in prop::collection::vec(prompt_strategy(), 0..12),
        times in prop::collection::vec(commit_strategy(), 0..20),
    ) {
        let rows: Vec<FactRow> = prompts
            .iter()
            .enumerate()
            .map(|(i, &(minute, length, committed, session))| {
                story(i as i64 + 1, f64::from(minute), "p")
                    .session(Some(&format!("s{session}")))
                    .with(|r| {
                        r.response_s = length.map(f64::from);
                        r.git_commits = i64::from(committed);
                    })
                    .build()
            })
            .collect();
        let commits: Vec<Commit> = times
            .iter()
            .enumerate()
            .map(|(i, minute)| commit(&format!("c{i}"), &minute.map_or_else(|| "?".into(), |m| at(f64::from(m)))))
            .collect();
        let traces = commit_traces(&rows, &commits);
        prop_assert_eq!(traces.iter().map(|t| &t.commit).collect::<Vec<_>>(), commits.iter().collect::<Vec<_>>());
        prop_assert!(traces.iter().all(|t| t.prompt.is_none() == (t.how == TraceHow::ByHand)));
        let (forward, backward) = both_ways(&rows, &commits);
        prop_assert_eq!(forward, backward);
    }
}

// --- the project's commit list ---------------------------------------------------------------------

/// Two phases (a gap of more than three hours), branches main then fix; commits by hand before,
/// during the first turn, and during the second.
fn project() -> (Vec<FactRow>, Vec<Commit>) {
    let rows = vec![story(1, 0.0, "add the form").build(), story(2, 300.0, "fix the bug").branch(Some("fix")).build()];
    let mut readme = commit("aaa1111", &at(-30.0));
    readme.subject = "Write the README".into();
    readme.paths = vec!["README.md".into()];
    let mut form = commit("bbb2222", &at(0.5));
    form.subject = "Add the form".into();
    form.paths = vec!["src/form.rs".into()];
    let mut bug = commit("ccc3333", &at(300.5));
    bug.subject = "Fix the bug".into();
    bug.paths = vec!["src/form.rs".into()];
    (rows, vec![readme, form, bug])
}

fn list(group: CommitGrouping, q: &str) -> ProjectCommits {
    let (rows, commits) = project();
    project_commits("/r/app", &rows, &commits, &CommitQuery { q: q.into(), group })
}

/// `(key, hashes)` of each group.
fn groups(found: &ProjectCommits) -> Vec<(Option<String>, Vec<String>)> {
    found.groups.iter().map(|g| (g.key.clone(), g.commits.iter().map(|t| t.commit.hash.clone()).collect())).collect()
}

#[test]
fn the_list_counts_every_commit_and_groups_them_by_phase_newest_first() {
    let found = list(CommitGrouping::Phase, "");
    assert_eq!(found.counts, TraceCounts { total: 3, traced: 2, likely: 0, by_hand: 1 });
    assert_eq!(found.matched, 3);
    assert_eq!(
        groups(&found),
        [(Some("2".into()), vec!["ccc3333".into()]), (Some("1".into()), vec!["bbb2222".into()]), (None, vec!["aaa1111".into()])]
    );
    assert_eq!(found.groups.iter().map(|g| g.count).collect::<Vec<_>>(), [1, 1, 1]);
    assert_eq!((found.project.name.as_str(), found.project.git), ("acme/app", true));
}

#[test]
fn the_list_groups_by_the_prompt_branch_or_the_commit_day() {
    let by_branch = groups(&list(CommitGrouping::Branch, ""));
    assert_eq!(
        by_branch,
        [(Some("fix".into()), vec!["ccc3333".into()]), (Some("main".into()), vec!["bbb2222".into()]), (None, vec!["aaa1111".into()])]
    );
    let by_day = groups(&list(CommitGrouping::Day, ""));
    assert_eq!(by_day, [(Some("2026-03-02".into()), vec!["ccc3333".into(), "bbb2222".into(), "aaa1111".into()])]);
}

#[test]
fn a_search_matches_the_subject_a_path_or_the_hash_and_keeps_the_counts_whole() {
    let hashes = |q: &str| groups(&list(CommitGrouping::Day, q)).into_iter().flat_map(|(_, hashes)| hashes).collect::<Vec<String>>();
    assert_eq!(hashes("readme"), ["aaa1111"]);
    assert_eq!(hashes("form.rs"), ["ccc3333", "bbb2222"]);
    assert_eq!(hashes("bbb2"), ["bbb2222"]);
    assert_eq!(hashes("bbb2222f00dfeedbeef"), ["bbb2222"]);
    assert_eq!(hashes("FIX form"), ["ccc3333"]);
    assert_eq!(hashes("nothing"), Vec::<String>::new());
    let found = list(CommitGrouping::Day, "readme");
    assert_eq!((found.matched, found.counts.total), (1, 3));
}
