//! Commit traces (`stories/commits.rs`, `stories/commit_list.rs`): each commit with the prompt behind
//! it, how sure that is, and the project's commit list searched and grouped. One test per kind of
//! work that fooled the first rule on a real repository (ticket 14): an amend, a rebase, two
//! sessions at once, a commit by hand between prompts, a worktree, a subagent. The property test
//! checks the one promise everything else rests on: reading the rule from the commits' side gives
//! exactly what `attribute_commits` gives from the prompts' side, and only a guess names another
//! prompt.
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
    COMMIT_GRACE_S, CommitGrouping, CommitQuery, CommitTrace, LIKELY_S, LOG_LEAD_S, ProjectCommits, TraceCounts, TraceHow,
    attribute_commits, commit_traces, project_commits, trace_window,
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

// --- what real work produces ----------------------------------------------------------------------

/// A commit whose change was made at `authored` and committed (again) at `committed`.
fn rewritten(hash: &str, authored: f64, committed: f64) -> Commit {
    Commit { authored: at(authored), ..commit(hash, &at(committed)) }
}

/// The alternative prompt each trace names.
fn alternatives(traces: &[CommitTrace]) -> Vec<Option<i64>> {
    traces.iter().map(|t| t.alternative.as_ref().map(|a| a.prompt.id)).collect()
}

#[test]
fn a_commit_amended_later_keeps_the_prompt_it_was_made_in_but_only_as_likely() {
    let rows = [story(1, 0.0, "add the form").build(), story(2, 60.0, "explain the cache").build()];
    let traces = commit_traces(&rows, &[rewritten("amended", 0.5, 60.5)]);
    assert_eq!(read(&traces), [("amended".into(), Some(1), TraceHow::Likely)]);
}

#[test]
fn a_rebase_that_changed_hashes_traces_each_commit_to_the_work_it_was_made_in() {
    let rows = [committing(1, 0.0, "s", 1.0), committing(2, 10.0, "s", 1.0)];
    // Rebased by hand at 30, after both turns; a rebase inside the turn's grace stays certain.
    let commits = [rewritten("form", 0.5, 30.0), rewritten("bug", 10.5, 30.0), rewritten("quick", 10.5, 12.0)];
    assert_eq!(
        read(&commit_traces(&rows, &commits)),
        [
            ("form".into(), Some(1), TraceHow::Likely),
            ("bug".into(), Some(2), TraceHow::Likely),
            ("quick".into(), Some(2), TraceHow::Traced)
        ]
    );
}

#[test]
fn two_sessions_running_in_one_repository_make_the_commit_likely_and_name_both_unless_one_alone_ran_git_commit() {
    let asking = |id: i64, minutes: f64, session: &str| story(id, minutes, "q").session(Some(session)).response_s(Some(600.0)).build();
    let trace_at_8 = |rows: &[FactRow]| {
        let traces = commit_traces(rows, &[commit("c", &at(8.0))]);
        (read(&traces).remove(0), alternatives(&traces).remove(0), traces[0].alternative.as_ref().and_then(|a| a.session.clone()))
    };
    // Neither ran `git commit`, or both did: the later sent, naming the other.
    let neither = [asking(1, 0.0, "a"), asking(2, 5.0, "b")];
    assert_eq!(trace_at_8(&neither), (("c".into(), Some(2), TraceHow::Likely), Some(1), Some("a".into())));
    let both = [committing(1, 0.0, "a", 10.0), committing(2, 5.0, "b", 10.0)];
    assert_eq!(trace_at_8(&both), (("c".into(), Some(2), TraceHow::Likely), Some(1), Some("a".into())));
    // Only one did: it made the commit.
    let one = [committing(1, 0.0, "a", 10.0), asking(2, 5.0, "b")];
    assert_eq!(trace_at_8(&one), (("c".into(), Some(1), TraceHow::Traced), None, None));
}

#[test]
fn a_commit_by_hand_between_two_prompts_is_likely_soon_after_the_work_and_by_hand_long_after() {
    // The one-minute turns end at 1 and at 601.
    let rows = [story(1, 0.0, "a").build(), story(2, 600.0, "b").build()];
    let likely = 1.0 + LIKELY_S as f64 / 60.0;
    let commits = [
        commit("soon", &at(30.0)),
        commit("edge", &at(likely)),
        commit("late", &at(likely + 1.0)),
        commit("week", &at(601.0 + 7.0 * 1440.0)),
    ];
    assert_eq!(
        read(&commit_traces(&rows, &commits)),
        [
            ("soon".into(), Some(1), TraceHow::Likely),
            ("edge".into(), Some(1), TraceHow::Likely),
            ("late".into(), None, TraceHow::ByHand),
            ("week".into(), None, TraceHow::ByHand)
        ]
    );
}

#[test]
fn a_commit_by_hand_while_another_session_runs_without_committing_is_likely_the_work_that_left_changes() {
    // The first work wrote files, never committed, and ended at 10; the second runs from 5 to 25.
    let rows = [
        story(1, 0.0, "make the ci cheaper").session(Some("a")).response_s(Some(600.0)).build(),
        story(2, 5.0, "fix the console warnings").session(Some("b")).response_s(Some(1200.0)).build(),
    ];
    let traces = commit_traces(&rows, &[commit("ci", &at(20.0))]);
    assert_eq!((read(&traces), alternatives(&traces)), (vec![("ci".into(), Some(1), TraceHow::Likely)], vec![Some(2)]));
    // Had the first work committed its changes itself, the running one made the commit.
    let rows =
        [committing(1, 0.0, "a", 10.0), story(2, 5.0, "fix the console warnings").session(Some("b")).response_s(Some(1200.0)).build()];
    assert_eq!(read(&commit_traces(&rows, &[commit("ci", &at(20.0))])), [("ci".into(), Some(2), TraceHow::Traced)]);
}

#[test]
fn a_commit_in_a_worktree_belongs_to_the_turn_running_there_not_to_a_later_prompt_in_the_main_checkout() {
    let rows = [
        story(1, 0.0, "build the parser").session(Some("w")).branch(Some("worktree-parser")).response_s(Some(1200.0)).build(),
        story(2, 10.0, "what does main do").session(Some("m")).with(|r| r.files_written = 0).build(),
    ];
    let traces = commit_traces(&rows, &[commit("c", &at(15.0))]);
    assert_eq!(
        (read(&traces), traces[0].prompt.as_ref().unwrap().branch.as_deref()),
        (vec![("c".into(), Some(1), TraceHow::Traced)], Some("worktree-parser"))
    );
}

#[test]
fn a_commit_by_a_subagent_belongs_to_the_prompt_that_started_it_until_the_hand_back() {
    let rows = [
        story(1, 0.0, "build it with a subagent").session(Some("a")).build(),
        story(2, 5.0, "a question elsewhere").session(Some("b")).with(|r| r.files_written = 0).build(),
        story(3, 40.0, "<agent-message> done").session(Some("a")).origin("agent_message").response_s(Some(120.0)).build(),
    ];
    // The work of prompt 1 runs to the end of the hand-back's turn (42), plus the grace.
    let commits = [commit("sub", &at(30.0)), commit("back", &at(42.0 + grace())), commit("after", &at(50.0))];
    assert_eq!(
        read(&commit_traces(&rows, &commits)),
        [
            ("sub".into(), Some(1), TraceHow::Traced),
            ("back".into(), Some(1), TraceHow::Traced),
            ("after".into(), Some(1), TraceHow::Likely)
        ]
    );
    assert_eq!(attribute_commits(&rows, &commits).keys().copied().collect::<Vec<_>>(), [1]);
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

/// A prompt: minute sent, turn length in seconds (none: unknown), whether it committed, session,
/// whether you typed it (else a subagent's hand-back).
fn prompt_strategy() -> impl Strategy<Value = (u16, Option<u16>, bool, u8, bool)> {
    (0..600u16, prop::option::of(0..1200u16), any::<bool>(), 0..3u8, prop::bool::weighted(0.7))
}

/// A commit's minute made, and committed again that many minutes later (0: never rewritten).
fn rewrite_strategy() -> impl Strategy<Value = u16> {
    prop::sample::select(vec![0u16, 0, 0, 1, 30, 300])
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
        times in prop::collection::vec((commit_strategy(), rewrite_strategy()), 0..20),
    ) {
        let rows: Vec<FactRow> = prompts
            .iter()
            .enumerate()
            .map(|(i, &(minute, length, committed, session, typed))| {
                story(i as i64 + 1, f64::from(minute), "p")
                    .session(Some(&format!("s{session}")))
                    .origin(if typed { "typed" } else { "agent_message" })
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
            .map(|(i, &(minute, later))| match minute {
                Some(m) => rewritten(&format!("c{i}"), f64::from(m), f64::from(m) + f64::from(later)),
                None => commit(&format!("c{i}"), "?"),
            })
            .collect();
        let traces = commit_traces(&rows, &commits);
        prop_assert_eq!(traces.iter().map(|t| &t.commit).collect::<Vec<_>>(), commits.iter().collect::<Vec<_>>());
        prop_assert!(traces.iter().all(|t| t.prompt.is_none() == (t.how == TraceHow::ByHand)));
        // Only a prompt you typed owns a commit, and only a guess names another one.
        let typed = |id: i64| usize::try_from(id - 1).ok().and_then(|i| prompts.get(i)).is_some_and(|p| p.4);
        prop_assert!(traces.iter().filter_map(|t| t.prompt.as_ref()).all(|p| typed(p.id)));
        for trace in &traces {
            if let (Some(prompt), Some(other)) = (&trace.prompt, &trace.alternative) {
                prop_assert!(trace.how == TraceHow::Likely && other.prompt.id != prompt.id && typed(other.prompt.id));
            }
            prop_assert!(trace.alternative.is_none() || trace.prompt.is_some());
        }
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
