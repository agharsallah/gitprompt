"""Prompt stories: phases, retries, commit attribution and story assembly (synthetic rows)."""

from datetime import UTC, datetime, timedelta

import pytest

from gitprompt.insights import stories
from gitprompt.insights.stories import attribute_commits, group_markers, phases, project_id, project_repo, retries

T0 = datetime(2026, 3, 2, 9, 0, tzinfo=UTC)


def at(minutes: float) -> str:
    return (T0 + timedelta(minutes=minutes)).isoformat()


def row(id, minutes, text="do something", session="s1", branch="main", origin="typed", **extra):
    """A prompt as storage/timeline.py returns it."""
    return {
        "id": id, "prompt_id": f"p{id}", "source": "claude-code", "session": session, "repo": "/r/app", "repo_name": "acme/app",
        "branch": branch, "title": None, "received_at": at(minutes), "origin": origin, "typed_text": text, "chars": len(text),
        "text_hash": text.lower(), "slash_command": None, "code_blocks": 0, "mentions": 0, "urls": 0, "cost_usd": 0.5,
        "api_requests": 1, "input_tokens": 10, "output_tokens": 5, "response_s": 60.0, "ended_by": "Stop", "tool_calls": 2,
        "tool_failures": 0, "subagents": 0, "files_written": 1, "git_commits": 0, "compactions": 0, "outcome": "done", **extra,
    }  # fmt: skip


def reasons(found):
    return [(p["reason"], [r["id"] for r in p["prompts"]]) for p in found]


def test_project_ids_round_trip_and_are_url_safe():
    for repo in ("/Users/me/code/app", "/tmp/a b/ü", "C:\\work\\x?y"):
        key = project_id(repo)
        assert project_repo(key) == repo and all(c.isalnum() or c in "-_" for c in key)
    assert project_repo("%%%") is None and project_repo("_w") is None


def test_a_long_pause_starts_a_phase():
    # turns last a minute: the pause is measured from the end of the previous turn
    found = phases([row(1, 0), row(2, 10), row(3, 10 + 1 + 180), row(4, 10 + 1 + 180 + 179)])
    assert reasons(found) == [("start", [1, 2]), ("gap", [3, 4])]
    assert found[1]["gap_s"] == 3 * 3600


def test_a_branch_starts_a_phase_once_it_holds():
    found = phases([row(1, 0), row(2, 1, branch="feat"), row(3, 2), row(4, 3, branch="feat"), row(5, 4, branch="feat")])
    assert reasons(found) == [("start", [1, 2, 3]), ("branch", [4, 5])]  # prompt 2 was a detour


def test_prompts_without_a_branch_keep_the_phase():
    found = phases([row(1, 0, branch=None), row(2, 1), row(3, 2, branch=None), row(4, 3, branch="x"), row(5, 4, branch="x")])
    assert reasons(found) == [("start", [1, 2, 3]), ("branch", [4, 5])]
    assert phases([]) == []


def test_retries_find_the_earlier_wording_in_the_same_session():
    found = retries(
        [
            row(1, 0, "add tests for the parser"),
            row(2, 1, "explain the cache"),
            row(3, 2, "add unit tests for the parser module"),
            row(4, 3, "add tests for the parser", session="s2"),  # another session: not a retry
        ]
    )
    assert found == {3: 1}


def test_a_commit_belongs_to_the_turn_that_made_it():
    prompts = [
        row(1, 0, session="a", git_commits=1, response_s=600),  # long turn that commits
        row(2, 5, session="b"),  # sent meanwhile, from a parallel session
        row(3, 30, session="b"),
    ]
    commits = [{"hash": h, "time": at(m), "subject": h} for h, m in (("c0", -5), ("c1", 9), ("c2", 20), ("c3", 31))]
    owned = attribute_commits(prompts, commits)
    assert {k: [c["hash"] for c in v] for k, v in owned.items()} == {1: ["c1"], 2: ["c2"], 3: ["c3"]}  # c0: before any prompt


def test_markers_between_chapters_fold_into_groups():
    items = [{"kind": "chapter"}, {"kind": "task_notification"}, {"kind": "compaction"}, {"kind": "chapter"}, {"kind": "agent_message"}]
    assert [(i["kind"], len(i.get("items", []))) for i in group_markers(items)] == [
        ("chapter", 0),
        ("markers", 2),
        ("chapter", 0),
        ("markers", 1),
    ]


def test_session_story_never_shows_system_text_as_typed():
    rows = [
        row(1, 0, "fix the flaky test"),
        row(2, 3, "<task-notification>build done</task-notification>", origin="task_notification"),
        row(3, 4, "fix the flaky tests please", outcome="failed", ended_by="StopFailure"),
    ]
    events = [
        {"prompt_id": "p1", "event": "PostToolUse", "received_at": at(1), "tool": "Edit",
         "tool_input": '{"file_path": "/r/app/a.py"}', "prompt": None},
        {"prompt_id": "p1", "event": "PostToolUse", "received_at": at(2), "tool": "Agent",
         "tool_input": '{"prompt": "Find the test helpers"}',
         "description": "Look around", "subagent_type": "Explore", "prompt": "Find the test helpers"},
        {"prompt_id": "p3", "event": "PreCompact", "received_at": at(5), "tool": None, "tool_input": None, "prompt": None},
    ]  # fmt: skip
    commits = [{"hash": "abc", "subject": "Fix test", "author": "t", "time": at(2), "files": 1, "insertions": 3, "deletions": 1}]
    story = stories.session_story("s1", rows, {1: {"write": 1, "agent": 1}}, events, commits, {"Edit": "write", "Agent": "agent"})

    kinds = [i["kind"] for i in story["items"]]
    assert kinds == ["chapter", "markers", "chapter", "markers"]
    first, system, retry, tail = story["items"]
    assert first["text"] == "fix the flaky test" and first["files"] == ["/r/app/a.py"] and first["tools"] == {"write": 1, "agent": 1}
    assert first["diffstat"] == {"commits": 1, "files": 1, "insertions": 3, "deletions": 1}
    assert [(m["kind"], m["id"]) for m in system["items"]] == [("subagent_prompt", 1), ("task_notification", 2)]
    assert system["items"][0] | {"received_at": ""} == {
        "kind": "subagent_prompt", "id": 1, "received_at": "", "label": "Look around", "detail": "Explore",
        "excerpt": "Find the test helpers", "clipped": False,
    }  # fmt: skip
    assert retry["retry_of"] == 1 and retry["outcome"] == "failed" and retry["turn"] == 3
    assert tail["items"][0]["kind"] == "compaction"
    assert all(i["origin"] in stories.USER_ORIGINS for i in story["items"] if i["kind"] == "chapter")
    assert story["totals"] | {"cost_usd": 0} == {
        "chapters": 2, "markers": 3, "cost_usd": 0, "tool_calls": 6, "files_written": 1,
        "outcomes": {"done": 1, "failed": 1}, "retries": 1,
        "diffstat": {"commits": 1, "files": 1, "insertions": 3, "deletions": 1},
    }  # fmt: skip
    assert story["project"] == {"id": project_id("/r/app"), "repo": "/r/app", "name": "acme/app", "git": True}


def test_long_texts_are_clipped():
    story = stories.session_story("s1", [row(1, 0, "x" * (stories.CHAPTER_CHARS + 10))], {}, [], [], {})
    chapter = story["items"][0]
    assert chapter["clipped"] and len(chapter["text"]) == stories.CHAPTER_CHARS + 1


def test_project_story_phases_and_measures():
    rows = [
        row(1, 0, "set up the project", mentions=1),
        row(2, 5, "add the parser", outcome="superseded", ended_by=None, response_s=None),
        row(3, 6, "add the parser module"),
        row(4, 60 * 24, "write the docs", session="s2", code_blocks=1),
    ]
    commits = [{"hash": "c1", "subject": "Parser", "author": "t", "time": at(8), "files": 2, "insertions": 10, "deletions": 0}]
    story = stories.project_story("/r/app", rows, {}, commits, system=7)

    assert story["id"] == project_id("/r/app") and story["system"] == 7
    assert [(p["reason"], [e["id"] for e in p["items"]]) for p in story["phases"]] == [("start", [1, 2, 3]), ("gap", [4])]
    first = story["phases"][0]
    assert first["outcomes"] == {"done": 2, "superseded": 1} and first["retries"] == 1 and first["with_mention"] == 1
    assert first["diffstat"] == {"commits": 1, "files": 2, "insertions": 10, "deletions": 0}
    assert first["items"][2]["commits"][0]["hash"] == "c1" and first["items"][2]["retry_of"] == 2
    assert story["totals"]["prompts"] == 4 and story["totals"]["sessions"] == 2 and story["totals"]["with_code"] == 1


def test_project_story_keeps_only_what_you_typed():
    rows = [row(1, 0), row(2, 1, "<agent-message>hi</agent-message>", origin="agent_message")]
    story = stories.project_story("/r/app", rows, {}, [])
    assert [e["id"] for p in story["phases"] for e in p["items"]] == [1]


def test_lineage_orders_versions_and_says_what_changed():
    target = row(3, 10, "add unit tests for the parser module", session="s1")
    candidates = [
        row(1, 0, "add tests for the parser", session="s1", outcome="failed"),
        row(2, 5, "add tests for the parser", session="s9", repo="/r/other", repo_name=None),
        row(4, 20, "explain the cache", session="s1"),  # unrelated
        row(5, 30, "add tests for the parser", origin="task_notification"),  # never a version
        target,
    ]
    found = stories.prompt_lineage(target, candidates)
    versions = found["versions"]
    assert [v["id"] for v in versions] == [1, 2, 3]
    assert [v["place"] for v in versions] == ["session", "elsewhere", "self"]
    assert versions[0]["diff"] is None and versions[1]["change"] == "improved" and versions[2]["change"] == "same"
    assert ("insert", "unit ") in versions[2]["diff"] or any(op == "insert" and "unit" in t for op, t in versions[2]["diff"])
    assert versions[1]["project"]["name"] == "other" and found["threshold"] == pytest.approx(0.5)


def test_a_system_prompt_has_no_lineage():
    target = row(1, 0, "<task-notification>x</task-notification>", origin="task_notification")
    assert stories.prompt_lineage(target, [target, row(2, 1, "<task-notification>x</task-notification>")])["versions"] == []
