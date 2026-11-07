"""Prompt stories (plan E4, ADR-0016): ordered, explained views of what you typed, for one session or
one project. Built from rows of storage/timeline.py and commits from core/git.history; no I/O here,
so an export (E5) can render the same story to another format.

    phases(prompts)                  a project's prompts split where you paused or changed branch
    retries(prompts)                 prompt id -> the earlier prompt of its session it rewords
    attribute_commits(prompts, log)  prompt id -> the commits made after it
    session_story(...)               typed prompts as chapters, system items as markers between them
    project_story(...)               phases of typed prompts with commits, cost and outcomes

A story never shows system-originated text as typed: only `USER_ORIGINS` become chapters or project
prompts; everything else is a marker, labelled by its origin.
"""

import base64
import binascii
import json
import re
import statistics
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from datetime import datetime, timedelta
from typing import Any

from gitprompt.insights import lineage
from gitprompt.insights.derivations import files_of
from gitprompt.sources.spec import tool_kind

USER_ORIGINS = ("typed", "slash_command")
# A pause this long starts a new phase. On the local data the gaps between prompts of a project fall
# into minutes (one sitting, parallel sessions interleaved) and many hours (another sitting): 3 h
# sits in the valley between them (ADR-0016).
GAP_S = 3 * 3600
# A branch starts a new phase once it holds for this many prompts in a row: a single prompt sent
# from another branch is a detour of the current phase, not a phase of its own.
BRANCH_RUN = 2
RETRY_WINDOW = 30  # earlier prompts of the session compared when looking for a retry
COMMIT_GRACE_S = 120  # a commit this soon after a turn ended still belongs to it
LOG_TAIL_S = 2 * 3600  # commits read after the last prompt of a story
CHAPTER_CHARS = 4000  # typed text shown in a session chapter (the prompt detail has all of it)
EXCERPT_CHARS = 280  # in a project story, a marker or a search result
MAX_FILES = 20  # file paths listed per chapter


# --- project ids -------------------------------------------------------------------------------


def project_id(repo: str) -> str:
    """A stable, URL-safe key for a project (git root or working directory): base64url, no padding.
    Reversible, so no lookup table is needed and the dashboard can build it too."""
    return base64.urlsafe_b64encode(repo.encode()).rstrip(b"=").decode()


def project_repo(key: str) -> str | None:
    """The git root or folder a project id names; None if it is not one."""
    try:
        return base64.b64decode(key + "=" * (-len(key) % 4), altchars=b"-_", validate=True).decode() or None
    except (binascii.Error, UnicodeDecodeError, ValueError):
        return None


# --- pure building blocks ----------------------------------------------------------------------


def _time(iso: str) -> datetime:
    return datetime.fromisoformat(iso)


def _ended(p: dict[str, Any]) -> datetime:
    return _time(p["received_at"]) + timedelta(seconds=p.get("response_s") or 0)


def phases(prompts: Sequence[dict[str, Any]], gap_s: float = GAP_S, branch_run: int = BRANCH_RUN) -> list[dict[str, Any]]:
    """Split prompts (oldest first) into phases: a new phase starts after a pause of `gap_s` or more
    (`reason` gap, with `gap_s`), or when the branch changes and the new branch holds for
    `branch_run` prompts (`reason` branch). Prompts without a branch never change it."""
    out: list[dict[str, Any]] = []
    branch: str | None = None
    for i, p in enumerate(prompts):
        reason, gap = None, None
        if not out:
            reason = "start"
        else:
            gap = (_time(p["received_at"]) - _ended(prompts[i - 1])).total_seconds()
            run = [q.get("branch") for q in prompts[i : i + branch_run]]
            if gap >= gap_s:
                reason = "gap"
            elif branch and p.get("branch") and p["branch"] != branch and len(run) == branch_run and len(set(run)) == 1:
                reason = "branch"
        if reason:
            out.append({"reason": reason, "gap_s": round(gap) if reason == "gap" and gap is not None else None, "prompts": []})
            branch = p.get("branch") or branch
        branch = branch or p.get("branch")
        out[-1]["prompts"].append(p)
    return out


def retries(prompts: Sequence[dict[str, Any]], window: int = RETRY_WINDOW) -> dict[int, int]:
    """prompt id -> id of the latest earlier prompt of the same session that it rewords or repeats
    (lineage.is_version), among the `window` before it."""
    by_session: dict[Any, list[tuple[int, lineage.Shingles]]] = {}
    out: dict[int, int] = {}
    for p in prompts:
        earlier = by_session.setdefault(p.get("session"), [])
        mine = lineage.Shingles(p.get("typed_text") or "")
        for pid, other in reversed(earlier[-window:]):
            if lineage.is_version(mine, other):
                out[p["id"]] = pid
                break
        earlier.append((p["id"], mine))
    return out


def attribute_commits(prompts: Sequence[dict[str, Any]], commits: Iterable[dict[str, Any]]) -> dict[int, list[dict[str, Any]]]:
    """prompt id -> the commits made after it (oldest first). A commit belongs to the latest prompt
    sent before it, unless a turn that ran `git commit` was still going (or had just ended) when it
    was made: then to that turn. That keeps parallel sessions apart."""
    ordered = sorted(prompts, key=lambda p: p["received_at"])
    out: dict[int, list[dict[str, Any]]] = {}
    for c in commits:
        try:
            at = _time(c["time"])
        except (KeyError, TypeError, ValueError):
            continue
        before = [p for p in ordered if _time(p["received_at"]) <= at]
        if not before:
            continue
        committing = [p for p in before if p.get("git_commits") and at <= _ended(p) + timedelta(seconds=COMMIT_GRACE_S)]
        owner = (committing or before)[-1]
        out.setdefault(owner["id"], []).append(c)
    return out


def diffstat(commits: Iterable[dict[str, Any]]) -> dict[str, int] | None:
    """Commits, files, insertions and deletions summed; None without commits."""
    commits = list(commits)
    if not commits:
        return None
    return {"commits": len(commits), **{k: sum(c.get(k, 0) for c in commits) for k in ("files", "insertions", "deletions")}}


def log_window(prompts: Sequence[dict[str, Any]]) -> tuple[str, str] | None:
    """(since, until) of the commits a story reads: from its first prompt to a while after its last."""
    if not prompts:
        return None
    last = max(_ended(p) for p in prompts)
    return prompts[0]["received_at"], (last + timedelta(seconds=LOG_TAIL_S)).isoformat()


def group_markers(items: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    """Consecutive markers folded into one `markers` item, so system activity between two chapters
    reads as one collapsed group."""
    out: list[dict[str, Any]] = []
    for item in items:
        if item["kind"] == "chapter":
            out.append(item)
        elif out and out[-1]["kind"] == "markers":
            out[-1]["items"].append(item)
        else:
            out.append({"kind": "markers", "items": [item]})
    return out


def _clip(text: str | None, chars: int) -> tuple[str, bool]:
    text = text or ""
    return (text, False) if len(text) <= chars else (text[:chars].rstrip() + "…", True)


def is_user(p: dict[str, Any]) -> bool:
    return p.get("origin") in USER_ORIGINS


# --- a session ---------------------------------------------------------------------------------


def _chapter(p: dict[str, Any], turn: int, kinds: dict[str, int], files: list[str], commits: list[dict], retry_of: int | None) -> dict:
    text, clipped = _clip(p.get("typed_text"), CHAPTER_CHARS)
    return {
        "kind": "chapter",
        "id": p["id"],
        "turn": turn,
        "received_at": p["received_at"],
        "origin": p["origin"],
        "text": text,
        "clipped": clipped,
        "slash_command": p.get("slash_command"),
        "branch": p.get("branch"),
        "outcome": p["outcome"],
        "response_s": p.get("response_s"),
        "cost_usd": p.get("cost_usd") or 0,
        "tokens": (p.get("input_tokens") or 0) + (p.get("output_tokens") or 0),
        "tool_calls": p.get("tool_calls") or 0,
        "tool_failures": p.get("tool_failures") or 0,
        "tools": kinds,
        "files_written": p.get("files_written") or 0,
        "files": files[:MAX_FILES],
        "subagents": p.get("subagents") or 0,
        "git_commits": p.get("git_commits") or 0,
        "commits": commits,
        "diffstat": diffstat(commits),
        "retry_of": retry_of,
        "facts": p.get("facts"),  # insights/facts.py, attached by the caller
    }


def _marker(
    kind: str, p: dict[str, Any], received_at: str, label: str | None = None, detail: str | None = None, text: str | None = None
) -> dict:
    excerpt, clipped = _clip(text, EXCERPT_CHARS)
    return {
        "kind": kind,
        "id": p["id"],
        "received_at": received_at,
        "label": label,
        "detail": detail,
        "excerpt": excerpt or None,
        "clipped": clipped,
    }


def _turn_markers(p: dict[str, Any], events: list[dict[str, Any]], kinds: Mapping[str, str]) -> list[dict]:
    """Subagents the turn started (what the model asked them) and its compactions."""
    out = []
    for e in events:
        if e["event"] == "PreCompact":
            out.append(_marker("compaction", p, e["received_at"]))
        elif tool_kind(kinds, e["tool"]) == "agent" and e.get("prompt"):
            out.append(_marker("subagent_prompt", p, e["received_at"], e.get("description"), e.get("subagent_type"), e["prompt"]))
    return out


def _files(events: list[dict[str, Any]], kinds: Mapping[str, str], tool_files: Mapping[str, re.Pattern[str]]) -> list[str]:
    """Files the turn's write tools touched (storage.timeline gives each call's input as JSON text)."""
    writes = [e for e in events if tool_kind(kinds, e["tool"]) == "write" and e.get("tool_input")]
    return sorted({f for e in writes for f in files_of(e["tool"], json.loads(e["tool_input"]), tool_files)})


def session_story(
    session: str,
    rows: Sequence[dict[str, Any]],
    tool_kinds: dict[int, dict[str, int]],
    events: Iterable[dict[str, Any]],
    commits: Iterable[dict[str, Any]],
    kinds: Mapping[str, str],
    tool_files: Mapping[str, re.Pattern[str]] | None = None,
) -> dict[str, Any]:
    """One session as a narrative: each prompt you typed is a chapter (outcome, tools, files, commits,
    cost, time); task notifications, agent messages, subagent prompts and compactions are markers
    between chapters, folded into groups."""
    by_turn: dict[str, list[dict[str, Any]]] = {}
    for e in events:
        by_turn.setdefault(e["prompt_id"], []).append(e)
    user = [p for p in rows if is_user(p)]
    owned = attribute_commits(user, commits)
    retry = retries(user)
    items: list[dict[str, Any]] = []
    for turn, p in enumerate(rows, start=1):
        turn_events = by_turn.get(p.get("prompt_id"), [])
        if is_user(p):
            files = _files(turn_events, kinds, tool_files or {})
            items.append(_chapter(p, turn, tool_kinds.get(p["id"], {}), files, owned.get(p["id"], []), retry.get(p["id"])))
        else:
            items.append(_marker(p["origin"], p, p["received_at"], text=p.get("typed_text")))
        items += _turn_markers(p, turn_events, kinds)
    chapters = [i for i in items if i["kind"] == "chapter"]
    first = rows[0] if rows else {}
    ended = max((_ended(p) for p in rows), default=None)
    return {
        "session": session,
        "title": next((p["title"] for p in rows if p.get("title")), None),
        "source": first.get("source"),
        "connector": (first.get("facts") or {}).get("connector"),
        "models": dict(Counter(m for p in rows if (m := (p.get("facts") or {}).get("model")))),
        "project": project_of(rows),
        "branches": list(dict.fromkeys(p["branch"] for p in rows if p.get("branch"))),
        "started_at": first.get("received_at"),
        "ended_at": ended and ended.isoformat(),
        "totals": {
            "chapters": len(chapters),
            "markers": len(items) - len(chapters),
            "cost_usd": round(sum(p.get("cost_usd") or 0 for p in rows), 4),
            "tool_calls": sum(p.get("tool_calls") or 0 for p in rows),
            "files_written": len({f for i in chapters for f in i["files"]}),
            "outcomes": dict(Counter(i["outcome"] for i in chapters)),
            "retries": len(retry),
            "diffstat": diffstat(c for i in chapters for c in i["commits"]),
        },
        "items": group_markers(items),
    }


def project_of(rows: Sequence[dict[str, Any]]) -> dict[str, Any] | None:
    repo = next((p["repo"] for p in rows if p.get("repo")), None)
    if not repo:
        return None
    name = next((p["repo_name"] for p in rows if p.get("repo_name")), None) or repo.rstrip("/").rsplit("/", 1)[-1]
    return {"id": project_id(repo), "repo": repo, "name": name, "git": any(p.get("branch") for p in rows)}


# --- a project ---------------------------------------------------------------------------------


def _entry(p: dict[str, Any], kinds: dict[str, int], commits: list[dict], retry_of: int | None) -> dict[str, Any]:
    text, clipped = _clip(p.get("typed_text"), EXCERPT_CHARS)
    return {
        "id": p["id"],
        "received_at": p["received_at"],
        "session": p.get("session"),
        "origin": p["origin"],
        "branch": p.get("branch"),
        "text": text,
        "clipped": clipped,
        "chars": p.get("chars") or 0,
        "outcome": p["outcome"],
        "response_s": p.get("response_s"),
        "cost_usd": p.get("cost_usd") or 0,
        "tool_calls": p.get("tool_calls") or 0,
        "tool_failures": p.get("tool_failures") or 0,
        "tools": kinds,
        "files_written": p.get("files_written") or 0,
        "git_commits": p.get("git_commits") or 0,
        "commits": commits,
        "retry_of": retry_of,
        "facts": p.get("facts"),  # insights/facts.py, attached by the caller
    }


def _measures(entries: Sequence[dict[str, Any]], prompts: Sequence[dict[str, Any]]) -> dict[str, Any]:
    """What a phase (or the whole project) amounted to, and how its prompts were written."""
    commits = [c for e in entries for c in e["commits"]]
    return {
        "prompts": len(entries),
        "sessions": len({e["session"] for e in entries if e["session"]}),
        "cost_usd": round(sum(e["cost_usd"] for e in entries), 4),
        "tool_calls": sum(e["tool_calls"] for e in entries),
        "files_written": sum(e["files_written"] for e in entries),
        "outcomes": dict(Counter(e["outcome"] for e in entries)),
        "diffstat": diffstat(commits),
        "retries": sum(e["retry_of"] is not None for e in entries),
        "median_chars": round(statistics.median(e["chars"] for e in entries)) if entries else 0,
        "with_mention": sum((p.get("mentions") or 0) > 0 for p in prompts),
        "with_code": sum((p.get("code_blocks") or 0) > 0 for p in prompts),
        "first_at": entries[0]["received_at"] if entries else None,
        "last_at": entries[-1]["received_at"] if entries else None,
    }


def project_story(
    repo: str,
    rows: Sequence[dict[str, Any]],
    tool_kinds: dict[int, dict[str, int]],
    commits: Iterable[dict[str, Any]],
    system: int = 0,
) -> dict[str, Any]:
    """How a project got built, prompt by prompt: the prompts you typed there (oldest first), split
    into phases, each prompt with the commits made after it. `system`: prompts of other origins in
    the project, counted but never shown as typed."""
    user = [p for p in rows if is_user(p)]
    owned = attribute_commits(user, commits)
    retry = retries(user)
    entries = {p["id"]: _entry(p, tool_kinds.get(p["id"], {}), owned.get(p["id"], []), retry.get(p["id"])) for p in user}
    out_phases = []
    for n, phase in enumerate(phases(user), start=1):
        items = [entries[p["id"]] for p in phase["prompts"]]
        out_phases.append(
            {
                "index": n,
                "reason": phase["reason"],
                "gap_s": phase["gap_s"],
                "branches": list(dict.fromkeys(e["branch"] for e in items if e["branch"])),
                **_measures(items, phase["prompts"]),
                "items": items,
            }
        )
    project = project_of(user or rows) or {"id": project_id(repo), "repo": repo, "name": repo, "git": False}
    return {**project, "system": system, "totals": _measures(list(entries.values()), user), "phases": out_phases}


# --- a prompt's lineage ------------------------------------------------------------------------

MAX_VERSIONS = 20  # the prompt itself and its closest versions
VERSION_CHARS = 2000
# Candidates whose raw sizes differ more than this can't pass lineage.LENGTH_RATIO once normalized
# (normalizing removes a little): skipped before any shingling.
_SIZE_SLACK = 0.8


def _place(p: dict[str, Any], target: dict[str, Any]) -> str:
    if p["id"] == target["id"]:
        return "self"
    if p.get("session") and p.get("session") == target.get("session"):
        return "session"
    return "project" if p.get("repo") == target.get("repo") else "elsewhere"


def prompt_lineage(target: dict[str, Any], candidates: Iterable[dict[str, Any]]) -> dict[str, Any]:
    """The versions of a typed prompt, oldest first: itself and the prompts you typed whose text is
    similar enough (lineage.THRESHOLD), each with how its wording changed from the version before
    and whether its outcome was better. A system-originated prompt has no lineage."""
    if not is_user(target):
        return {"id": target["id"], "threshold": lineage.THRESHOLD, "versions": []}
    mine = lineage.Shingles(target.get("typed_text") or "")
    size = max(target.get("chars") or 0, 1)
    scored = []
    for c in candidates:
        chars = c.get("chars") or 0
        if c["id"] == target["id"] or not is_user(c) or min(chars, size) / max(chars, size, 1) < lineage.LENGTH_RATIO * _SIZE_SLACK:
            continue
        if (score := lineage.similarity(mine, lineage.Shingles(c.get("typed_text") or ""))) >= lineage.THRESHOLD:
            scored.append((score, c))
    kept = sorted(scored, key=lambda s: -s[0])[: MAX_VERSIONS - 1] + [(1.0, target)]
    versions, previous = [], None
    for score, p in sorted(kept, key=lambda s: (s[1]["received_at"], s[1]["id"])):
        text, clipped = _clip(p.get("typed_text"), VERSION_CHARS)
        versions.append(
            {
                "id": p["id"],
                "received_at": p["received_at"],
                "session": p.get("session"),
                "project": project_of([p]),
                "origin": p["origin"],
                "text": text,
                "clipped": clipped,
                "similarity": score,
                "same_text": bool(p.get("text_hash")) and p.get("text_hash") == target.get("text_hash"),
                "place": _place(p, target),
                "outcome": p["outcome"],
                "response_s": p.get("response_s"),
                "cost_usd": p.get("cost_usd") or 0,
                "tool_failures": p.get("tool_failures") or 0,
                "diff": lineage.word_diff(previous["typed_text"], p.get("typed_text")) if previous else None,
                "change": lineage.change(previous["outcome"], p["outcome"]) if previous else None,
            }
        )
        previous = p
    return {"id": target["id"], "threshold": lineage.THRESHOLD, "versions": versions}
