import os
import subprocess

from gitprompt.core.git import commits, git_context, history, shortstat


def git(repo, *args):
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True, capture_output=True)


def test_context_and_commits_after_a_prompt(tmp_path):
    git(tmp_path, "init", "-b", "main")
    assert git_context(str(tmp_path))["git_branch"] == "main"  # before the first commit
    (tmp_path / "a.txt").write_text("a")
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-m", "first")
    git(tmp_path, "remote", "add", "origin", "https://user:secret@github.com/acme/widgets.git")
    (tmp_path / "b.txt").write_text("b")

    ctx = git_context(str(tmp_path))
    assert ctx["git_head_subject"] == "first" and ctx["git_untracked"] == 1 and ctx["git_staged"] == 0
    assert ctx["git_repo"] == "acme/widgets" and ctx["git_host"] == "github.com" and "secret" not in ctx["git_remote"]

    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-m", "second")
    found = commits(str(tmp_path), ctx["git_head"], "HEAD", "2026-01-01")
    assert [c["subject"] for c in found["commits"]] == ["second"] and "1 file changed" in found["diffstat"]
    assert git_context(str(tmp_path / "missing")) == {}


def test_history_lists_commits_of_a_window_with_their_diffstat(tmp_path):
    git(tmp_path, "init", "-b", "main")
    for name, when in (("a", "2026-01-01T10:00:00+00:00"), ("b", "2026-01-02T10:00:00+00:00"), ("c", "2026-01-03T10:00:00+00:00")):
        (tmp_path / f"{name}.txt").write_text("one\ntwo\n")
        git(tmp_path, "add", ".")
        subprocess.run(
            ["git", "-C", str(tmp_path), "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-m", f"add {name}"],
            check=True,
            capture_output=True,
            env={**os.environ, "GIT_COMMITTER_DATE": when, "GIT_AUTHOR_DATE": when},
        )
    found = history(str(tmp_path), "2026-01-01T12:00:00+00:00", "2026-01-03T12:00:00+00:00")
    assert [c["subject"] for c in found] == ["add b", "add c"]  # oldest first, inside the window
    assert found[0] | {"hash": "", "time": ""} == {
        "hash": "", "subject": "add b", "author": "t", "time": "", "files": 1, "insertions": 2, "deletions": 0
    }  # fmt: skip
    assert history(str(tmp_path / "missing"), "2026-01-01", "2026-01-02") == []


def test_shortstat_reads_each_part():
    assert shortstat(" 3 files changed, 10 insertions(+), 1 deletion(-)") == {"files": 3, "insertions": 10, "deletions": 1}
    assert shortstat(" 1 file changed, 2 deletions(-)") == {"files": 1, "insertions": 0, "deletions": 2}
    assert shortstat("") == {"files": 0, "insertions": 0, "deletions": 0}
