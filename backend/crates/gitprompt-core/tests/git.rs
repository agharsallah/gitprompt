//! Git facts from real repositories in temp dirs (port of tests/core/test_git.py, plus worktrees,
//! bare repositories, detached HEAD, upstream counts, conflicts and the GitCache).

mod common;

use std::path::Path;
use std::time::Duration;

use common::{DATE, Repo, git, git_output, repo};
use gitprompt_core::git::{Commit, CommitsAfter, GitCache, Shortstat, commits, git_context, history, shortstat};
use gitprompt_core::{Json, JsonMap};
use pretty_assertions::assert_eq;
use serde_json::json;

fn context(path: &Path) -> Json {
    Json::Object(git_context(Some(path)))
}

fn root(repo: &Repo) -> String {
    repo.root.display().to_string()
}

/// A repository with one commit `first` (a.txt) at [`DATE`].
fn committed() -> Repo {
    let repo = repo();
    repo.commit("a.txt", "a\n", "first", DATE);
    repo
}

// --- git_context ----------------------------------------------------------------------------

#[test]
fn a_repository_without_commits_reports_its_branch_and_zero_counts() {
    let repo = repo();
    let expected =
        json!({"git_root": root(&repo), "git_branch": "main", "git_staged": 0, "git_modified": 0, "git_untracked": 0, "git_conflicts": 0});
    assert_eq!(context(&repo.root), expected);
}

#[test]
fn a_committed_repository_reports_its_head_commit() {
    let repo = committed();
    let expected = json!({
        "git_root": root(&repo), "git_branch": "main", "git_head": repo.head(12),
        "git_staged": 0, "git_modified": 0, "git_untracked": 0, "git_conflicts": 0,
        "git_head_subject": "first", "git_head_time": DATE, "git_head_author": "t"
    });
    assert_eq!(context(&repo.root), expected);
}

#[test]
fn a_subdirectory_reports_the_repository_root() {
    let repo = committed();
    repo.write("src/deep/x.txt", "x");
    assert_eq!(context(&repo.root.join("src/deep"))["git_root"], json!(root(&repo)));
}

#[test]
fn staged_modified_and_untracked_files_are_counted() {
    let repo = repo();
    repo.write("tracked.txt", "1\n");
    repo.write("both.txt", "1\n");
    repo.git(&["add", "-A"]);
    git(&repo.root, &["commit", "-q", "-m", "base"]);
    repo.write("tracked.txt", "2\n"); // modified, not staged
    repo.write("both.txt", "2\n");
    repo.git(&["add", "both.txt"]);
    repo.write("both.txt", "3\n"); // staged and modified again
    repo.write("new.txt", "n"); // staged new file
    repo.git(&["add", "new.txt"]);
    repo.write("u1.txt", "u");
    repo.write("u2.txt", "u");
    let ctx = context(&repo.root);
    let counts = [&ctx["git_staged"], &ctx["git_modified"], &ctx["git_untracked"], &ctx["git_conflicts"]];
    assert_eq!(counts, [&json!(2), &json!(2), &json!(2), &json!(0)]);
}

#[test]
fn context_and_commits_after_a_prompt() {
    let repo = committed();
    repo.git(&["remote", "add", "origin", "https://user:secret@git.example.com/acme/widgets.git"]);
    repo.write("b.txt", "b");

    let ctx = git_context(Some(&repo.root));
    assert_eq!((&ctx["git_head_subject"], &ctx["git_untracked"], &ctx["git_staged"]), (&json!("first"), &json!(1), &json!(0)));
    let remote = [&ctx["git_remote"], &ctx["git_repo"], &ctx["git_host"]];
    assert_eq!(remote, [&json!("https://git.example.com/acme/widgets.git"), &json!("acme/widgets"), &json!("git.example.com")]);

    let start = ctx["git_head"].as_str().unwrap().to_string();
    repo.commit("b.txt", "b\n", "second", "2026-01-02T10:00:00+02:00");
    let found = commits(&repo.root, Some(&start), "HEAD", "2026-01-01");
    let subjects: Vec<&str> = found.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["second"]);
    assert!(found.diffstat.as_deref().unwrap_or_default().contains("1 file changed"), "{found:?}");
}

#[test]
fn credentials_in_the_remote_url_are_never_kept() {
    let repo = committed();
    repo.git(&["remote", "add", "origin", "https://x-access-token:ghs_abc@git.example.com/acme/widgets"]);
    let ctx = git_context(Some(&repo.root));
    assert_eq!(ctx["git_remote"], json!("https://git.example.com/acme/widgets"));
    assert!(!Json::Object(ctx).to_string().contains("ghs_abc"));
}

#[test]
fn an_ssh_remote_gives_repo_and_host() {
    let repo = committed();
    repo.git(&["remote", "add", "origin", "git@git.example.com:acme/widgets.git"]);
    let ctx = context(&repo.root);
    let remote = [&ctx["git_remote"], &ctx["git_repo"], &ctx["git_host"]];
    assert_eq!(remote, [&json!("git@git.example.com:acme/widgets.git"), &json!("acme/widgets"), &json!("git.example.com")]);
}

#[test]
fn a_remote_with_a_port_and_trailing_slash_gives_repo_and_host() {
    let repo = committed();
    repo.git(&["remote", "add", "origin", "https://git.example.com:8443/group/project/"]);
    let ctx = context(&repo.root);
    assert_eq!((&ctx["git_repo"], &ctx["git_host"]), (&json!("group/project"), &json!("git.example.com")));
}

#[test]
fn a_repository_without_origin_has_no_remote_keys() {
    let repo = committed();
    repo.git(&["remote", "add", "upstream", "https://git.example.com/a/b.git"]);
    let ctx = git_context(Some(&repo.root));
    let present: Vec<&str> = ["git_remote", "git_repo", "git_host"].into_iter().filter(|k| ctx.contains_key(*k)).collect();
    assert_eq!(present, Vec::<&str>::new());
}

#[test]
fn a_detached_head_is_flagged_and_has_no_branch() {
    let repo = committed();
    repo.git(&["checkout", "-q", "--detach"]);
    let ctx = git_context(Some(&repo.root));
    assert_eq!((ctx.get("git_detached"), ctx.get("git_branch")), (Some(&json!(true)), None));
    assert_eq!(ctx["git_head"], json!(repo.head(12)));
}

#[test]
fn an_attached_head_leaves_the_detached_flag_out() {
    let ctx = git_context(Some(&committed().root));
    assert_eq!(ctx.get("git_detached"), None);
}

#[test]
fn upstream_and_ahead_behind_counts_are_reported() {
    let repo = committed();
    repo.git(&["branch", "base"]);
    repo.commit("main.txt", "m\n", "on main", "2026-01-02T10:00:00+02:00");
    repo.git(&["checkout", "-q", "base"]);
    repo.commit("base1.txt", "b\n", "on base 1", "2026-01-02T10:00:00+02:00");
    repo.commit("base2.txt", "b\n", "on base 2", "2026-01-02T11:00:00+02:00");
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["branch", "-q", "--set-upstream-to=base"]);
    let ctx = context(&repo.root);
    assert_eq!((&ctx["git_upstream"], &ctx["git_ahead"], &ctx["git_behind"]), (&json!("base"), &json!(1), &json!(2)));
}

#[test]
fn an_up_to_date_upstream_keeps_zero_ahead_and_behind() {
    let repo = committed();
    repo.git(&["branch", "base"]);
    repo.git(&["branch", "-q", "--set-upstream-to=base"]);
    let ctx = context(&repo.root);
    assert_eq!((&ctx["git_ahead"], &ctx["git_behind"]), (&json!(0), &json!(0)));
}

#[test]
fn merge_conflicts_are_counted() {
    let repo = committed();
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.commit("a.txt", "other\n", "other change", DATE);
    repo.git(&["checkout", "-q", "main"]);
    repo.commit("a.txt", "main\n", "main change", DATE);
    let merge = git_output(&repo.root, DATE, &["merge", "-q", "other"]);
    assert!(!merge.status.success(), "the merge conflicts");
    assert_eq!(context(&repo.root)["git_conflicts"], json!(1));
}

#[test]
fn a_linked_worktree_belongs_to_its_main_repository() {
    let repo = committed();
    let worktree = repo.sibling("wt");
    repo.git(&["worktree", "add", "-q", "-b", "feature", worktree.to_str().unwrap()]);
    let ctx = git_context(Some(&worktree));
    let layout = [&ctx["git_root"], &ctx["git_worktree"], &ctx["git_worktree_root"], &ctx["git_branch"]];
    assert_eq!(layout, [&json!(root(&repo)), &json!(true), &json!(worktree.display().to_string()), &json!("feature")]);
}

#[test]
fn the_main_working_tree_is_not_flagged_as_a_worktree() {
    let repo = committed();
    repo.git(&["worktree", "add", "-q", "-b", "feature", repo.sibling("wt").to_str().unwrap()]);
    let ctx = git_context(Some(&repo.root));
    assert_eq!((ctx.get("git_worktree"), ctx.get("git_worktree_root")), (None, None));
}

#[test]
fn a_worktree_of_a_bare_repository_belongs_to_the_bare_folder() {
    let repo = committed();
    let bare = repo.sibling("bare.git");
    git(repo.root.parent().unwrap(), &["clone", "-q", "--bare", repo.root.to_str().unwrap(), bare.to_str().unwrap()]);
    let worktree = repo.sibling("bare-wt");
    git(&bare, &["worktree", "add", "-q", worktree.to_str().unwrap(), "main"]);
    let ctx = git_context(Some(&worktree));
    let layout = [&ctx["git_root"], &ctx["git_worktree"], &ctx["git_worktree_root"]];
    assert_eq!(layout, [&json!(bare.display().to_string()), &json!(true), &json!(worktree.display().to_string())]);
}

#[test]
fn outside_a_repository_the_context_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(git_context(Some(dir.path())), JsonMap::new());
}

#[test]
fn a_missing_directory_or_no_directory_gives_an_empty_context() {
    let repo = committed();
    assert_eq!(git_context(Some(&repo.root.join("missing"))), JsonMap::new());
    assert_eq!(git_context(None), JsonMap::new());
}

// --- commits --------------------------------------------------------------------------------

#[test]
fn commits_after_a_known_head_are_listed_newest_first_with_a_diffstat() {
    let repo = committed();
    let start = repo.head(12);
    repo.commit("b.txt", "1\n2\n", "second", "2026-01-02T10:00:00+02:00");
    let second = repo.short_head();
    repo.commit("c.txt", "1\n", "third", "2026-01-03T10:00:00+02:00");
    let third = repo.short_head();
    let found = commits(&repo.root, Some(&start), "HEAD", "2026-01-01");
    let expected = CommitsAfter {
        commits: vec![
            Commit {
                hash: third,
                subject: "third".into(),
                author: "t".into(),
                time: "2026-01-03T10:00:00+02:00".into(),
                ..Commit::default()
            },
            Commit {
                hash: second,
                subject: "second".into(),
                author: "t".into(),
                time: "2026-01-02T10:00:00+02:00".into(),
                ..Commit::default()
            },
        ],
        diffstat: Some("2 files changed, 3 insertions(+)".into()),
        until: "HEAD".into(),
    };
    assert_eq!(found, expected);
}

#[test]
fn without_a_known_head_commits_since_the_prompt_time_are_listed_without_diffstat() {
    let repo = committed(); // 2026-01-01
    repo.commit("b.txt", "b\n", "second", "2026-01-03T10:00:00+02:00");
    let found = commits(&repo.root, None, "HEAD", "2026-01-02T00:00:00+02:00");
    let subjects: Vec<&str> = found.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!((subjects, found.diffstat, found.until.as_str()), (vec!["second"], None, "HEAD"));
}

#[test]
fn no_commits_after_the_prompt_gives_only_until() {
    let repo = committed();
    let found = commits(&repo.root, Some(&repo.head(12)), "HEAD", "2026-01-01");
    assert_eq!(found, CommitsAfter { commits: vec![], diffstat: None, until: "HEAD".into() });
    assert_eq!(serde_json::to_value(&found).unwrap(), json!({"until": "HEAD"}));
}

#[test]
fn commits_outside_a_repository_are_empty() {
    let dir = tempfile::tempdir().unwrap();
    let found = commits(dir.path(), Some("abc123"), "HEAD", "2026-01-01");
    assert_eq!(found, CommitsAfter { commits: vec![], diffstat: None, until: "HEAD".into() });
}

// --- shortstat / history --------------------------------------------------------------------

#[test]
fn shortstat_reads_each_part() {
    assert_eq!(shortstat(" 3 files changed, 10 insertions(+), 1 deletion(-)"), Shortstat { files: 3, insertions: 10, deletions: 1 });
    assert_eq!(shortstat(" 1 file changed, 2 deletions(-)"), Shortstat { files: 1, insertions: 0, deletions: 2 });
    assert_eq!(shortstat(" 1 file changed, 1 insertion(+)"), Shortstat { files: 1, insertions: 1, deletions: 0 });
    assert_eq!(shortstat(""), Shortstat::default());
    assert_eq!(shortstat("nothing to see"), Shortstat::default());
}

/// Commits a, b, c on main, one per day from 2026-01-01T10:00, each adding two lines.
fn three_days() -> Repo {
    let repo = repo();
    for (name, when) in [("a", "2026-01-01T10:00:00+02:00"), ("b", "2026-01-02T10:00:00+02:00"), ("c", "2026-01-03T10:00:00+02:00")] {
        repo.commit(&format!("{name}.txt"), "one\ntwo\n", &format!("add {name}"), when);
    }
    repo
}

#[test]
fn history_lists_commits_of_a_window_with_their_diffstat() {
    let repo = three_days();
    let found = history(&repo.root, "2026-01-01T12:00:00+02:00", "2026-01-03T12:00:00+02:00");
    let subjects: Vec<&str> = found.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["add b", "add c"]); // oldest first, inside the window
    let b = Commit { hash: String::new(), time: String::new(), ..found[0].clone() };
    assert_eq!(
        b,
        Commit {
            hash: String::new(),
            subject: "add b".into(),
            author: "t".into(),
            time: String::new(),
            files: 1,
            insertions: 2,
            deletions: 0
        }
    );
    assert_eq!((found[1].time.as_str(), found[1].hash.len() >= 7), ("2026-01-03T10:00:00+02:00", true));
}

#[test]
fn history_counts_deletions() {
    let repo = three_days();
    repo.commit("a.txt", "one\n", "trim a", "2026-01-04T10:00:00+02:00");
    let found = history(&repo.root, "2026-01-04T00:00:00+02:00", "2026-01-05T00:00:00+02:00");
    let stats: Vec<(String, u64, u64, u64)> = found.into_iter().map(|c| (c.subject, c.files, c.insertions, c.deletions)).collect();
    assert_eq!(stats, [("trim a".to_string(), 1, 0, 1)]);
}

#[test]
fn history_includes_every_local_branch() {
    let repo = three_days();
    repo.git(&["checkout", "-q", "-b", "side"]);
    repo.commit("s.txt", "s\n", "on side", "2026-01-04T10:00:00+02:00");
    repo.git(&["checkout", "-q", "main"]);
    let found = history(&repo.root, "2026-01-03T00:00:00+02:00", "2026-01-05T00:00:00+02:00");
    let subjects: Vec<&str> = found.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["add c", "on side"]);
}

#[test]
fn history_outside_a_repository_is_empty() {
    let repo = three_days();
    assert_eq!(history(&repo.root.join("missing"), "2026-01-01", "2026-01-02"), Vec::<Commit>::new());
}

// --- GitCache -------------------------------------------------------------------------------

#[test]
fn a_second_call_within_the_ttl_does_not_run_git_again() {
    let repo = committed();
    let cache = GitCache::new(Duration::from_secs(600));
    let first = cache.context(Some(&repo.root));
    repo.write("new.txt", "n"); // not seen until the entry expires
    let second = cache.context(Some(&repo.root));
    assert_eq!((cache.misses(), &second["git_untracked"]), (1, &json!(0)));
    assert_eq!(second, first);
    assert_eq!(git_context(Some(&repo.root))["git_untracked"], json!(1)); // the repository did change
}

#[test]
fn an_expired_entry_runs_git_again_and_sees_changes() {
    let repo = committed();
    let cache = GitCache::new(Duration::from_nanos(1));
    assert_eq!(cache.context(Some(&repo.root))["git_untracked"], json!(0));
    repo.write("new.txt", "n");
    assert_eq!(cache.context(Some(&repo.root))["git_untracked"], json!(1));
    assert_eq!(cache.misses(), 2);
}

#[test]
fn each_directory_is_cached_on_its_own() {
    let (one, two) = (committed(), committed());
    let cache = GitCache::new(Duration::from_secs(600));
    let a = cache.context(Some(&one.root));
    let b = cache.context(Some(&two.root));
    assert_eq!((a["git_root"].clone(), b["git_root"].clone(), cache.misses()), (json!(root(&one)), json!(root(&two)), 2));
}

#[test]
fn the_cache_gives_the_same_context_as_git_context() {
    let repo = committed();
    let cache = GitCache::new(Duration::from_secs(600));
    assert_eq!(cache.context(Some(&repo.root)), git_context(Some(&repo.root)));
    assert_eq!(cache.context(None), JsonMap::new());
}

#[test]
fn the_cache_is_shared_across_threads() {
    fn shareable<T: Send + Sync>() {}
    shareable::<GitCache>();
    let repo = committed();
    let cache = GitCache::new(Duration::from_secs(600));
    let expected = cache.context(Some(&repo.root));
    let found: Vec<JsonMap> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4).map(|_| scope.spawn(|| cache.context(Some(&repo.root)))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!((found, cache.misses()), (vec![expected; 4], 1));
}

#[test]
fn commits_after_a_prompt_serialize_without_diffstat_counts() {
    let after = gitprompt_core::git::CommitsAfter {
        commits: vec![gitprompt_core::git::Commit {
            hash: "abc1234".into(),
            subject: "fix".into(),
            author: "t".into(),
            time: "2026-11-01T10:00:00+02:00".into(),
            ..Default::default()
        }],
        diffstat: None,
        until: "def5678".into(),
    };
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::json!({
            "commits": [{"hash": "abc1234", "subject": "fix", "author": "t", "time": "2026-11-01T10:00:00+02:00"}],
            "until": "def5678"
        })
    );
}
