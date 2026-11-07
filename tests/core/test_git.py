import subprocess

from gitprompt.core.git import commits, git_context


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
