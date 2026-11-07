"""GET /api/stats: the prompt filters plus group_by, computed in SQL on the prompt index."""

import time
from typing import get_args

import pytest

from gitprompt.storage.stats import GROUPS, GroupBy

DASHBOARD = {"X-GitPrompt": "1"}


@pytest.fixture
def berlin(monkeypatch):
    monkeypatch.setenv("TZ", "Europe/Berlin")
    time.tzset()
    yield
    monkeypatch.undo()
    time.tzset()


@pytest.fixture
def data(client, settings, berlin):
    """Three sessions over two days, two repos, two models, every kind of ending (synthetic)."""
    headers = {"Authorization": f"Bearer {settings.token()}"}
    store = client.app.state.store

    def prompt(pid, text, session, at, cwd="/r/app"):
        payload = {"hook_event_name": "UserPromptSubmit", "prompt_id": pid, "session_id": session, "prompt": text, "cwd": cwd}
        assert client.post("/ingest/claude-code", json=payload, headers=headers).status_code == 204
        store.db.execute("update prompt set received_at = ? where json_extract(fields, '$.prompt_id') = ?", (at, pid))

    def event(pid, name, at, **raw):
        payload = {"hook_event_name": name, "prompt_id": pid, "session_id": "x", **raw}
        client.post("/ingest/claude-code", json=payload, headers=headers)
        store.db.execute("update event set received_at = ? where id = (select max(id) from event)", (at,))

    prompt("p1", "Fix the login bug", "s1", "2026-11-02T08:00:00+00:00")  # Monday 09:00 in Berlin
    prompt("p2", "/review", "s1", "2026-11-02T08:30:00+00:00")
    prompt("p3", "Write the docs", "s2", "2026-11-03T13:00:00+00:00", cwd="/r/docs")  # Tuesday 14:00
    prompt("p4", "<task-notification>done</task-notification>", "s3", "2026-11-03T13:05:00+00:00")
    event("p1", "PostToolUse", "2026-11-02T08:00:05+00:00", tool_name="Read", tool_input={"file_path": "/a"})
    event("p1", "PostToolUse", "2026-11-02T08:00:06+00:00", tool_name="Edit", tool_input={"file_path": "/a"})
    event("p1", "Stop", "2026-11-02T08:00:10+00:00")
    event("p2", "Stop", "2026-11-02T08:30:30+00:00")
    event("p3", "PostToolUse", "2026-11-03T13:00:05+00:00", tool_name="Bash", tool_input={"command": "make"})
    event("p3", "StopFailure", "2026-11-03T13:01:40+00:00")
    for pid, model, cost in (("p1", "opus", 1.0), ("p1", "haiku", 0.1), ("p3", "opus", 2.0)):
        event(pid, "api_request", "2026-11-03T13:00:01+00:00", model=model, cost_usd=cost, input_tokens=100)
    return client


def stats(client, **params):
    response = client.get("/api/stats", params=params)
    assert response.status_code == 200, response.text
    return response.json()


def keys(result, measure="prompts"):
    return {g["key"]: g[measure] for g in result["groups"]}


def test_group_by_names_match_the_groupings():
    assert set(get_args(GroupBy)) == GROUPS.keys()


def test_totals_follow_the_filters(data):
    everything = stats(data)["totals"]
    assert everything["prompts"] == 4 and everything["sessions"] == 3 and everything["cost_usd"] == pytest.approx(3.1)
    assert everything["api_requests"] == 3 and everything["input_tokens"] == 300 and everything["tool_calls"] == 3
    assert everything["ended"] == 3 and everything["response_s_p50"] == 30 and everything["response_s_p90"] == 86.0
    assert stats(data)["groups"] == []

    yours = stats(data, origin=["typed", "slash_command"])["totals"]
    assert yours["prompts"] == 3 and yours["sessions"] == 2
    assert stats(data, q="docs")["totals"]["prompts"] == 1
    assert stats(data, since="2026-11-03T00:00:00+00:00")["totals"]["prompts"] == 2
    assert stats(data, until="2026-11-03T00:00:00")["totals"]["prompts"] == 2  # no offset: local time
    assert stats(data, model=["haiku"])["totals"]["prompts"] == 1
    assert stats(data, repo="/r/docs")["totals"]["cost_usd"] == 2.0


@pytest.mark.parametrize(
    ("group_by", "expected"),
    [
        ("day", {"2026-11-02": 2, "2026-11-03": 2}),
        ("hour", {"09": 2, "14": 2}),
        ("weekday", {"1": 2, "2": 2}),
        ("weekday_hour", {"1-09": 2, "2-14": 2}),
        ("origin", {"typed": 2, "slash_command": 1, "task_notification": 1}),
        ("source", {"claude-code": 4}),
        ("session", {"s1": 2, "s2": 1, "s3": 1}),
        ("repo", {"/r/app": 3, "/r/docs": 1}),
        ("command", {None: 3, "/review": 1}),
        ("outcome", {"Stop": 2, "StopFailure": 1, None: 1}),
        ("model", {"opus": 2, "haiku": 1}),
        ("tool", {"Read": 1, "Edit": 1, "Bash": 1}),
        ("tool_kind", {"read": 1, "write": 1, "shell": 1}),
    ],
)
def test_each_group_by(data, group_by, expected):
    result = stats(data, group_by=group_by)
    assert result["group_by"] == group_by and keys(result) == expected
    if GROUPS[group_by].by_key:
        assert [g["key"] for g in result["groups"]] == sorted(expected)
    else:
        counts = [g["prompts"] for g in result["groups"]]
        assert counts == sorted(counts, reverse=True)


def test_usage_is_split_by_model_and_calls_by_tool_kind(data):
    by_model = stats(data, group_by="model")
    assert keys(by_model, "cost_usd") == {"opus": 3.0, "haiku": 0.1} and keys(by_model, "api_requests") == {"opus": 2, "haiku": 1}
    assert keys(stats(data, group_by="tool_kind"), "tool_calls") == {"read": 1, "write": 1, "shell": 1}
    by_day = stats(data, group_by="day")["groups"]
    assert by_day[0]["response_s_p50"] == 20.0 and by_day[1]["response_s_p90"] == 100.0


def test_limit_and_validation(data):
    assert len(stats(data, group_by="session", limit=1)["groups"]) == 1
    assert data.get("/api/stats?group_by=nope").status_code == 422
    assert data.get("/api/stats?origin=nope").status_code == 422


def test_prompt_list_takes_the_same_filters(data):
    listed = data.get("/api/prompts", params={"origin": ["typed", "slash_command"]}).json()
    assert [p["prompt"] for p in listed] == ["Write the docs", "/review", "Fix the login bug"]
    assert [p["prompt"] for p in data.get("/api/prompts", params={"origin": "task_notification"}).json()] == [
        "<task-notification>done</task-notification>"
    ]
    assert len(data.get("/api/prompts").json()) == 4  # no origin: all of them (backward compatible)
    assert [p["prompt"] for p in data.get("/api/prompts?q=LOGIN").json()] == ["Fix the login bug"]


def test_hiding_a_secret_also_removes_it_from_search(data, settings):
    headers = {"Authorization": f"Bearer {settings.token()}"}
    payload = {"hook_event_name": "UserPromptSubmit", "prompt_id": "p9", "session_id": "s9", "prompt": "key password=hunter2hunter2"}
    data.post("/ingest/claude-code", json=payload, headers=headers)
    [found] = data.get("/api/prompts?q=hunter2hunter2").json()
    data.post(f"/api/prompts/{found['id']}/redact", headers=DASHBOARD)
    assert data.get("/api/prompts?q=hunter2hunter2").json() == []


def test_remap_rebuilds_the_index(data):
    before = stats(data)["totals"]["prompts"]
    assert data.post("/api/sources/claude-code/remap", headers=DASHBOARD).status_code == 200
    assert stats(data)["totals"]["prompts"] == before
