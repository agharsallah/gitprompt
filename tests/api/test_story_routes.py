"""GET /api/stories/* and /api/search, end to end through ingest (synthetic payloads, temp dirs)."""

import os
import subprocess

import pytest

from gitprompt.insights.stories import project_id


def git(repo, *args, when=None):
    env = {**os.environ, **({"GIT_COMMITTER_DATE": when, "GIT_AUTHOR_DATE": when} if when else {})}
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True, capture_output=True, env=env)


@pytest.fixture
def repo(tmp_path):
    path = tmp_path / "app"
    path.mkdir()
    git(path, "init", "-b", "main")
    (path / "a.txt").write_text("a\n")
    git(path, "add", ".")
    git(path, "commit", "-m", "initial", when="2026-03-01T08:00:00+00:00")
    return path


@pytest.fixture
def data(client, settings, repo):
    """Two sessions in one repository: a retry, a task notification, a subagent, a commit."""
    headers = {"Authorization": f"Bearer {settings.token()}"}
    store = client.app.state.store

    def send(name, pid, session, at, **raw):
        payload = {"hook_event_name": name, "prompt_id": pid, "session_id": session, "cwd": str(repo), **raw}
        assert client.post("/ingest/claude-code", json=payload, headers=headers).status_code == 204
        table = "prompt" if name == "UserPromptSubmit" else "event"
        store.db.execute(f"update {table} set received_at = ? where id = (select max(id) from {table})", (at,))

    send("UserPromptSubmit", "p1", "s1", "2026-03-02T09:00:00+00:00", prompt="add tests for the parser")
    send("PostToolUse", "p1", "s1", "2026-03-02T09:00:10+00:00", tool_name="Edit", tool_input={"file_path": f"{repo}/a.txt"})
    send(
        "PostToolUse", "p1", "s1", "2026-03-02T09:00:20+00:00",
        tool_name="Agent", tool_input={"description": "Find helpers", "subagent_type": "Explore", "prompt": "List the test helpers"},
    )  # fmt: skip
    send("StopFailure", "p1", "s1", "2026-03-02T09:01:00+00:00")
    send("UserPromptSubmit", "p2", "s1", "2026-03-02T09:02:00+00:00", prompt="<task-notification>tests finished</task-notification>")
    send("UserPromptSubmit", "p3", "s1", "2026-03-02T09:03:00+00:00", prompt="add unit tests for the parser module")
    send("Stop", "p3", "s1", "2026-03-02T09:04:00+00:00")
    send("UserPromptSubmit", "p4", "s2", "2026-03-03T15:00:00+00:00", prompt="Write the release notes for the parser")
    send("Stop", "p4", "s2", "2026-03-03T15:01:00+00:00")
    (repo / "b.txt").write_text("b\nc\n")
    git(repo, "add", ".")
    git(repo, "commit", "-m", "Add parser tests", when="2026-03-02T09:03:30+00:00")
    return client


def get(client, path, **params):
    response = client.get(path, params=params)
    assert response.status_code == 200, response.text
    return response.json()


def prompt_ids(client):
    return {r["fields"]["prompt_id"]: r["id"] for r in client.get("/api/prompts", params={"limit": 50}).json()}


def test_session_story_has_chapters_and_markers(data, repo):
    ids = prompt_ids(data)
    story = get(data, "/api/stories/session/s1")
    assert [i["kind"] for i in story["items"]] == ["chapter", "markers", "chapter"]
    first, markers, retry = story["items"]
    assert first["id"] == ids["p1"] and first["outcome"] == "failed" and first["files"] == [f"{repo}/a.txt"]
    assert [(m["kind"], m["id"]) for m in markers["items"]] == [("subagent_prompt", ids["p1"]), ("task_notification", ids["p2"])]
    assert markers["items"][1]["excerpt"] == "<task-notification>tests finished</task-notification>"
    assert retry["retry_of"] == ids["p1"] and retry["outcome"] == "done" and retry["turn"] == 3
    assert [c["subject"] for c in retry["commits"]] == ["Add parser tests"] and retry["diffstat"]["insertions"] == 2
    assert story["project"]["id"] == project_id(str(repo)) and story["branches"] == ["main"]
    assert story["totals"]["chapters"] == 2 and story["totals"]["markers"] == 2


def test_project_story_has_phases(data, repo):
    story = get(data, f"/api/stories/project/{project_id(str(repo))}")
    assert story["repo"] == str(repo) and story["system"] == 1
    assert [(p["reason"], p["prompts"]) for p in story["phases"]] == [("start", 2), ("gap", 1)]
    assert story["totals"]["prompts"] == 3 and story["totals"]["diffstat"]["commits"] == 1
    assert all(not e["text"].startswith("<task-notification>") for p in story["phases"] for e in p["items"])


def test_unknown_stories_are_404(data):
    assert data.get("/api/stories/session/nope").status_code == 404
    assert data.get(f"/api/stories/project/{project_id('/nowhere')}").status_code == 404
    assert data.get("/api/stories/project/%25%25").status_code == 404
    assert data.get("/api/stories/prompt/9999").status_code == 404


def test_lineage_lists_versions_with_their_diff(data):
    ids = prompt_ids(data)
    lineage = get(data, f"/api/stories/prompt/{ids['p3']}")
    assert [(v["id"], v["place"], v["change"]) for v in lineage["versions"]] == [
        (ids["p1"], "session", None),
        (ids["p3"], "self", "improved"),
    ]
    assert any(op == "insert" and "unit" in text for op, text in lineage["versions"][1]["diff"])
    assert get(data, f"/api/stories/prompt/{ids['p2']}")["versions"] == []  # a task notification has no lineage


def test_search_returns_snippets_and_takes_the_filters(data, repo):
    found = get(data, "/api/search", q="PARSER tests")
    assert found["total"] == 2 and [h["snippet"] for h in found["hits"]][0] == [
        {"text": "add unit ", "hit": False},
        {"text": "tests", "hit": True},
        {"text": " for the ", "hit": False},
        {"text": "parser", "hit": True},
        {"text": " module", "hit": False},
    ]
    assert found["hits"][0]["project"]["id"] == project_id(str(repo))
    assert get(data, "/api/search", q="parser", origin=["typed"], since="2026-03-03T00:00:00+00:00")["total"] == 1
    assert get(data, "/api/search", q="tests finished")["total"] == 1  # every origin unless filtered
    assert get(data, "/api/search", q="tests finished", origin=["typed"])["total"] == 0
    assert get(data, "/api/search", q="parser", limit=1)["total"] == 3 and len(get(data, "/api/search", q="parser", limit=1)["hits"]) == 1
