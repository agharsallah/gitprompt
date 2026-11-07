"""Git facts for a working directory, read with the user's own git CLI (read-only commands)."""

import re
import subprocess
from typing import Any

SEP = "\x1f"  # field separator in --format strings


def _git(cwd: str, *args: str) -> str | None:
    """stdout of a git command, or None if git is missing, fails, times out or prints nothing."""
    try:
        out = subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True, timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return (out.stdout.strip() or None) if out.returncode == 0 else None


def _layout(cwd: str) -> dict[str, Any] | None:
    paths = _git(cwd, "rev-parse", "--path-format=absolute", "--show-toplevel", "--git-dir", "--git-common-dir")
    if not paths:
        return None
    root, git_dir, common = (paths.splitlines() + ["", ""])[:3]
    return {"git_root": root, "git_worktree": git_dir != common}


def _status(cwd: str) -> dict[str, Any]:
    """Branch, HEAD, upstream and working tree counts from `git status --porcelain=v2 --branch`."""
    lines = (_git(cwd, "status", "--porcelain=v2", "--branch") or "").splitlines()
    branch = dict(line[2:].split(" ", 1) for line in lines if line.startswith("# ") and " " in line[2:])
    entries = [line for line in lines if not line.startswith("#")]
    head, oid = branch.get("branch.head"), branch.get("branch.oid", "")
    ahead = behind = None
    if ab := branch.get("branch.ab"):
        ahead, behind = (int(n[1:]) for n in ab.split())
    return {
        "git_branch": head if head != "(detached)" else None,
        "git_detached": head == "(detached)",
        "git_head": oid[:12] if oid != "(initial)" else None,
        "git_upstream": branch.get("branch.upstream"),
        "git_ahead": ahead,
        "git_behind": behind,
        "git_staged": sum(e[0] in "12" and e[2] != "." for e in entries),
        "git_modified": sum(e[0] in "12" and e[3] != "." for e in entries),
        "git_untracked": sum(e[0] == "?" for e in entries),
        "git_conflicts": sum(e[0] == "u" for e in entries),
    }


def _remote(cwd: str) -> dict[str, Any]:
    url = _git(cwd, "remote", "get-url", "origin")
    if not url:
        return {}
    url = re.sub(r"//[^/@]+@", "//", url)  # never keep credentials embedded in a URL
    repo = re.search(r"[:/]([^/:]+/[^/]+?)(?:\.git)?/?$", url)
    host = re.search(r"(?:@|//)([^/:@]+)", url)
    return {"git_remote": url, "git_repo": repo and repo[1], "git_host": host and host[1]}


def _last_commit(cwd: str) -> dict[str, Any]:
    subject, time, author = ((_git(cwd, "log", "-1", f"--format=%s{SEP}%cI{SEP}%an") or "").split(SEP) + [None, None])[:3]
    return {"git_head_subject": subject or None, "git_head_time": time, "git_head_author": author}


def git_context(cwd: str | None) -> dict[str, Any]:
    """Repository state when the prompt was sent; {} outside a repository. Unknown values and false
    flags are left out; counts are kept even when 0."""
    layout = _layout(cwd) if cwd else None
    if not layout:
        return {}
    found = layout | _status(cwd) | _remote(cwd) | _last_commit(cwd)
    return {k: v for k, v in found.items() if v is not None and v != "" and v is not False}


def commits(root: str, start: str | None, end: str, since: str) -> dict[str, Any]:
    """Commits made after a prompt: `start..end` when the head at prompt time is known, otherwise
    everything on `end` since the prompt was sent (e.g. a repository that had no commits yet)."""
    rev = [f"{start}..{end}"] if start else [end, f"--since={since}"]
    log = _git(root, "log", f"--format=%h{SEP}%s{SEP}%an{SEP}%cI", *rev) or ""
    items = [dict(zip(("hash", "subject", "author", "time"), line.split(SEP), strict=False)) for line in log.splitlines()]
    stat = _git(root, "diff", "--shortstat", start, end) if start and items else None
    return {k: v for k, v in {"commits": items, "diffstat": stat, "until": end}.items() if v}
