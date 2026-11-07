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
        ("day_hour", {"2026-11-02 09": 2, "2026-11-03 14": 2}),
        ("hour", {"09": 2, "14": 2}),
        ("weekday", {"1": 2, "2": 2}),
        ("weekday_hour", {"1-09": 2, "2-14": 2}),
        ("origin", {"typed": 2, "slash_command": 1, "task_notification": 1}),
        ("source", {"claude-code": 4}),
        ("session", {"s1": 2, "s2": 1, "s3": 1}),
        ("repo", {"/r/app": 3, "/r/docs": 1}),
        ("session_length", {"00000": 1, "00060": 1, "01800": 2}),  # s3 no end; s2 100 s; s1 08:00 to 08:30:30
        ("size", {"00000": 3, "00020": 1}),  # typed characters
        ("work", {"main": 2}),
        ("query_source", {None: 2}),  # telemetry without a query_source: the main thread
        ("command", {None: 3, "/review": 1}),
        ("outcome", {"done": 2, "failed": 1, "open": 1}),
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


def ingest(client, settings, **payload):
    headers = {"Authorization": f"Bearer {settings.token()}"}
    assert client.post("/ingest/claude-code", json=payload, headers=headers).status_code == 204


def test_work_split_matches_the_totals(data, settings):
    for source, cost in (("repl_main_thread", 0.5), ("agent:builtin:Explore", 0.25), ("compact", 0.125), ("away_summary", 0.125)):
        ingest(
            data, settings, hook_event_name="api_request", prompt_id="p2", session_id="s1", model="opus", cost_usd=cost, query_source=source
        )
    work = stats(data, group_by="work")
    assert keys(work, "cost_usd") == {"main": pytest.approx(3.6), "subagent": 0.25, "system": 0.25}
    assert sum(g["cost_usd"] for g in work["groups"]) == pytest.approx(work["totals"]["cost_usd"])
    assert keys(stats(data, group_by="work"))["system"] == 1  # two system sources, one prompt
    by_source = stats(data, group_by="query_source")
    assert {g["key"]: g["detail"] for g in by_source["groups"]}["agent:builtin:Explore"] == "subagent"
    assert stats(data, query_source="compact")["totals"]["prompts"] == 1


def test_text_groups_reused_prompts_with_an_excerpt(data, settings):
    ingest(data, settings, hook_event_name="UserPromptSubmit", prompt_id="p5", session_id="s4", prompt="  fix the LOGIN   bug ")
    result = stats(data, group_by="text", origin=["typed", "slash_command"])
    top = result["groups"][0]
    assert top["prompts"] == 2 and top["detail"] == "fix the LOGIN   bug"
    assert result["totals"]["prompts"] - result["totals"]["distinct_texts"] == 1  # one prompt repeated a text
    assert stats(data, q="login")["totals"]["prompts"] == 2


def test_session_group_has_a_title_and_span(data):
    sessions = {g["key"]: g for g in stats(data, group_by="session")["groups"]}
    assert sessions["s1"]["detail"] == "Fix the login bug" and sessions["s1"]["span_s"] == 1830.0
    assert sessions["s3"]["span_s"] == 0.0


def test_order_by_a_measure(data):
    assert [g["key"] for g in stats(data, group_by="repo", order="cost_usd")["groups"]] == ["/r/docs", "/r/app"]
    assert [g["key"] for g in stats(data, group_by="session", order="last_at")["groups"]] == ["s3", "s2", "s1"]
    assert data.get("/api/stats?group_by=repo&order=nope").status_code == 422


def test_split_by_gives_each_group_its_parts(data):
    result = stats(data, group_by="repo", split_by="day")
    parts = {g["key"]: {p["key"]: p["prompts"] for p in g["parts"]} for g in result["groups"]}
    assert parts == {"/r/app": {"2026-11-02": 2, "2026-11-03": 1}, "/r/docs": {"2026-11-03": 1}}
    kinds = {
        g["key"]: {p["key"]: p["tool_calls"] for p in g["parts"]} for g in stats(data, group_by="repo", split_by="tool_kind")["groups"]
    }
    assert kinds == {"/r/app": {"read": 1, "write": 1}, "/r/docs": {"shell": 1}}
    by_outcome = stats(data, group_by="outcome", split_by="repo")["groups"]
    assert sum(p["prompts"] for g in by_outcome for p in g["parts"]) == 4
    assert data.get("/api/stats?group_by=model&split_by=tool").status_code == 422  # two multi-valued groupings
    assert data.get("/api/stats?group_by=repo&split_by=repo").status_code == 422
    assert data.get("/api/stats?split_by=repo").status_code == 422


def test_new_measures(data, settings):
    ingest(data, settings, hook_event_name="PostToolUse", prompt_id="p1", session_id="s1", tool_name="Bash", agent_id="a1",
           tool_input={"command": "git -C /r/app commit -m fix"}, duration_ms=1500)  # fmt: skip
    ingest(data, settings, hook_event_name="PermissionDenied", prompt_id="p3", session_id="s2", tool_name="Bash")
    ingest(data, settings, hook_event_name="PreCompact", prompt_id="p3", session_id="s2")
    ingest(data, settings, hook_event_name="SubagentStart", prompt_id="p1", session_id="s1", agent_type="Explore")
    ingest(data, settings, hook_event_name="UserPromptSubmit", prompt_id="p6", session_id="s5", prompt="see https://x.y and @README.md")
    t = stats(data)["totals"]
    assert (t["git_commits"], t["with_commit"], t["subagent_tool_calls"], t["subagents"]) == (1, 1, 1, 1)
    assert (t["denials"], t["compactions"], t["failed"], t["files_written"], t["tool_time_s"]) == (1, 1, 1, 1, 1.5)
    assert (t["with_url"], t["with_mention"], t["with_code"]) == (1, 1, 0)
    assert t["followed"] == 1  # p1 is followed by p2 in s1; every other session has one prompt


@pytest.mark.parametrize(
    ("params", "prompts"),
    [
        ({"weekday": 1}, 2),
        ({"hour": 14}, 2),
        ({"hour": 0}, 0),  # 0 is a value, not "no filter"
        ({"command": "/review"}, 1),
        ({"tool": "Bash"}, 1),
        ({"tool_kind": "write"}, 1),
        ({"outcome": ["done", "failed"]}, 3),
        ({"outcome": "open"}, 1),
        ({"size": "00020"}, 1),
        ({"has": "no_follow_up"}, 3),
        ({"session": ["s1", "s2"]}, 3),
        ({"session": "s2"}, 1),
    ],
)
def test_filters_match_their_groups(data, params, prompts):
    assert stats(data, **params)["totals"]["prompts"] == prompts
    assert len(data.get("/api/prompts", params=params).json()) == prompts


def test_outcome_counts_superseded_and_running(data, settings):
    ingest(data, settings, hook_event_name="UserPromptSubmit", prompt_id="p7", session_id="s3", prompt="and now?")
    by_outcome = keys(stats(data, group_by="outcome"))
    assert by_outcome == {"done": 2, "failed": 1, "superseded": 1, "running": 1}  # p4 had no end and p7 came after it


def test_telemetry_is_summed_over_the_kept_sessions(data, settings):
    metric = "claude_code.lines_of_code.count"
    ingest(data, settings, hook_event_name=metric, session_id="s1", type="added", value=10)
    ingest(data, settings, hook_event_name=metric, session_id="s2", type="added", value=5)
    ingest(data, settings, hook_event_name=metric, session_id="s9", type="added", value=100)  # no prompt: not counted
    ingest(data, settings, hook_event_name="user_prompt", session_id="s1", prompt_id="p8", command_name="model", command_source="builtin")
    response = data.get("/api/stats/telemetry", params={"origin": ["typed", "slash_command"]})
    assert response.status_code == 200, response.text
    result = response.json()
    assert result["sessions"] == 2 and result["metrics"] == {f"{metric}[added]": 15.0}
    assert result["commands"] == [{"command": "/model", "source": "builtin", "uses": 1, "sessions": 1}]
    assert data.get("/api/stats/telemetry", params={"repo": "/r/docs"}).json()["metrics"] == {f"{metric}[added]": 5.0}
    assert data.get("/api/stats/telemetry", params={"since": "2099-01-01T00:00:00Z"}).json()["metrics"] == {}


def test_filter_values_match_the_store():
    from gitprompt.api.filters import Has, Outcome
    from gitprompt.storage.store import HAS, OUTCOMES

    assert set(get_args(Has)) == HAS.keys() and get_args(Outcome) == OUTCOMES
