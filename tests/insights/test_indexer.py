"""The prompt index: filled from the same derivations as the API, updated when a turn gets events,
rebuilt when the rules change, searchable with FTS5 and filterable by origin (ADR-0012)."""

import sqlite3
import time

import pytest

from gitprompt.insights import indexer
from gitprompt.insights.indexer import Indexer
from gitprompt.sources.spec import load_specs
from gitprompt.storage.store import Store


@pytest.fixture
def berlin(monkeypatch):
    """Day and hour buckets use the machine's timezone: pin one with daylight saving."""
    monkeypatch.setenv("TZ", "Europe/Berlin")
    time.tzset()
    yield
    monkeypatch.undo()
    time.tzset()


@pytest.fixture
def store(settings):
    store = Store(settings.db_path)
    yield store
    store.close()


@pytest.fixture
def index(store, settings):
    return Indexer(store, load_specs(settings))


def add_prompt(store: Store, text: str, pid: str, session: str = "s1", at: str = "2026-11-03T10:00:00+00:00", **fields) -> int:
    row = store.add_prompt("claude-code", {"prompt": text, "prompt_id": pid, "session_id": session, **fields}, {}, {"prompt": text})
    store.db.execute("update prompt set received_at = ? where id = ?", (at, row["id"]))
    return row["id"]


def add_event(store: Store, event: str, pid: str, at: str = "2026-11-03T10:00:30+00:00", source: str = "claude-code", **raw) -> None:
    row = store.add_event(source, {"event": event, "prompt_id": pid, "session_id": "s1"}, raw)
    store.db.execute("update event set received_at = ? where id = ?", (at, row["id"]))
    store.index.mark([pid])  # as ingest does


def indexed(store: Store, id: int) -> dict:
    row = dict(store.db.execute("select * from prompt_index where id = ?", (id,)).fetchone())
    text = store.db.execute("select typed_text from prompt_fts where rowid = ?", (id,)).fetchone()
    return row | {"typed_text": text and text[0]}


def test_the_index_stores_what_the_api_derives(store, index, berlin):
    id = add_prompt(store, "<ide_selection>x</ide_selection>\nRename x", "p1", cwd="/work/app")
    add_event(store, "PostToolUse", "p1", tool_name="Edit", tool_input={"file_path": "/a.py"})
    add_event(store, "PostToolUse", "p1", tool_name="mcp__gh__issue")
    add_event(store, "api_request", "p1", source="claude-code-otel", model="opus", cost_usd=0.5, input_tokens=10)
    add_event(store, "api_request", "p1", source="claude-code-otel", model="haiku", cost_usd=0.25, output_tokens=4)
    add_event(store, "Stop", "p1", at="2026-11-03T10:01:00+00:00")
    assert index.ensure() == 1

    row = indexed(store, id)
    assert row["origin"] == "typed" and row["typed_text"] == "Rename x" and row["repo"] == "/work/app"
    assert (row["day"], row["hour"], row["weekday"]) == ("2026-11-03", 11, 2)  # 10:00 UTC is 11:00 in Berlin, a Tuesday
    assert row["cost_usd"] == 0.75 and row["api_requests"] == 2 and row["response_s"] == 60 and row["ended_by"] == "Stop"
    assert row["tool_calls"] == 2
    models = store.db.execute("select model, cost_usd, input_tokens from prompt_index_model order by model").fetchall()
    assert [tuple(m) for m in models] == [("haiku", 0.25, 0), ("opus", 0.5, 10)]
    tools = store.db.execute("select tool, kind, calls from prompt_index_tool order by tool").fetchall()
    assert [tuple(t) for t in tools] == [("Edit", "write", 1), ("mcp__gh__issue", "mcp", 1)]


def test_a_later_event_updates_its_prompt_only_once_read(store, index):
    id = add_prompt(store, "Fix it", "p1")
    other = add_prompt(store, "Other", "p2")
    index.ensure()
    assert indexed(store, id)["ended_by"] is None

    add_event(store, "Stop", "p1")
    add_event(store, "api_request", "p1", source="claude-code-otel", model="opus", cost_usd=1.5)
    assert indexed(store, id)["ended_by"] is None  # ingest only marks the turn...
    assert index.ensure() == 1  # ...the next read recomputes it, once, and nothing else
    assert indexed(store, id)["ended_by"] == "Stop" and indexed(store, id)["cost_usd"] == 1.5
    assert indexed(store, other)["ended_by"] is None and index.ensure() == 0


def test_events_before_their_prompt_are_counted_when_it_arrives(store, index):
    add_event(store, "user_prompt", "p1", source="claude-code-otel", command_name="/review")
    id = add_prompt(store, "Review this", "p1")
    index.ensure()
    assert indexed(store, id)["origin"] == "slash_command"


def test_a_rule_change_rebuilds_the_index(store, settings, index):
    id = add_prompt(store, "BUILD: nightly", "p1")
    index.ensure()
    assert indexed(store, id)["origin"] == "typed"

    settings.user_sources.mkdir(parents=True)
    spec = (indexer.Path(indexer.__file__).parents[1] / "sources" / "bundled" / "claude-code.yaml").read_text()
    rule = 'origin:\n  - when: {text_prefix: "BUILD:"}\n    origin: programmatic\n'
    (settings.user_sources / "claude-code.yaml").write_text(spec.replace("origin:\n", rule, 1))
    changed = Indexer(store, load_specs(settings))
    assert changed.version != index.version
    assert changed.ensure() == 1 and indexed(store, id)["origin"] == "programmatic"
    assert changed.ensure() == 0  # rebuilt once


def test_a_tool_kind_change_rebuilds_the_index(settings):
    specs = load_specs(settings)
    changed = {**specs, "claude-code": specs["claude-code"].model_copy(update={"tool_kinds": {"Bash": "web"}})}
    assert indexer.rules_version(changed) != indexer.rules_version(specs)


def test_a_code_change_rebuilds_the_index(store, settings, index, monkeypatch):
    add_prompt(store, "Fix it", "p1")
    index.ensure()
    monkeypatch.setattr(indexer, "CODE_VERSION", "changed")
    assert Indexer(store, load_specs(settings)).ensure() == 1


def test_the_index_can_be_dropped_and_rebuilt(store, index):
    id = add_prompt(store, "Fix it", "p1")
    index.ensure()
    store.index.drop()
    assert index.rebuild() == 1 and indexed(store, id)["typed_text"] == "Fix it"


def test_sqlite_has_fts5_with_trigrams():
    sqlite3.connect(":memory:").execute("create virtual table t using fts5(x, tokenize = 'trigram')")


@pytest.mark.parametrize(
    ("q", "found"),
    [
        ("login", ["Fix the Login bug"]),  # any case
        ("LOGIN fix", ["Fix the Login bug"]),  # every word, in any order
        ("ogi", ["Fix the Login bug"]),  # inside a word, like the old LIKE search
        ("go", ["go home"]),  # shorter than a trigram: checked without the index
        ("bug go", []),
        ('"quoted', ['say "quoted" things']),  # FTS5 syntax characters are plain text
        ("selection", []),  # injected context is not what you typed
        ("nothing-like-this", []),
    ],
)
def test_search_matches_words_in_the_typed_text(store, index, q, found):
    for i, text in enumerate(["Fix the Login bug", "go home", 'say "quoted" things', "<ide_selection>selection</ide_selection> Hi"]):
        add_prompt(store, text, f"p{i}")
    index.ensure()
    assert sorted(r["prompt"] for r in store.search(q=q)) == sorted(found)


def test_origin_and_time_filters(store, index):
    add_prompt(store, "Typed", "p1", at="2026-11-01T10:00:00+00:00")
    add_prompt(store, "/review", "p2", at="2026-11-02T10:00:00+00:00")
    add_prompt(store, "<task-notification>done</task-notification>", "p3", at="2026-11-03T10:00:00+00:00")
    index.ensure()

    def texts(**filters):
        return [r["prompt"] for r in store.search(**filters)]

    assert len(texts()) == 3 and len(texts(origin=[])) == 3  # no origin filter: every origin
    assert texts(origin=["typed", "slash_command"]) == ["/review", "Typed"]
    assert texts(origin=["task_notification"]) == ["<task-notification>done</task-notification>"]
    assert texts(since="2026-11-02T00:00:00+00:00", until="2026-11-03T00:00:00+00:00") == ["/review"]
    assert store.search(origin=["typed"])[0]["turn"] == 1  # rows still come from the view


def test_reindex_command(settings, store, capsys):
    from argparse import Namespace

    from gitprompt import cli

    add_prompt(store, "Fix it", "p1")
    cli.reindex(settings, Namespace())
    assert "Indexed 1 prompts" in capsys.readouterr().out


def test_turn_numbers_count_only_the_prompt_session(store, index):
    first = add_prompt(store, "One", "p1", session="a")
    add_prompt(store, "Other session", "p2", session="b")
    second = add_prompt(store, "Two", "p3", session="a")
    lone = [add_prompt(store, f"No session {i}", f"n{i}", session=None) for i in range(2)]
    index.ensure()
    assert store.get(second)["turn"] == 2 and store.get(second)["prev_at"] == store.get(first)["received_at"]
    assert store.get(first)["next_head"] is None and [store.get(i)["turn"] for i in lone] == [1, 2]
    assert [r["turn"] for r in store.search(session="a")] == [2, 1]


def test_the_index_keeps_what_the_insights_need(store, index):
    id = add_prompt(store, "<ide_selection>```py\nx\n```</ide_selection>\nSee https://a.b and @x.py", "p1", session_title="Docs")
    add_event(store, "PostToolUse", "p1", tool_name="Bash", tool_input={"command": "git add . && git commit -m x"}, agent_id="a1")
    add_event(store, "PostToolUse", "p1", tool_name="Bash", tool_input={"command": "git log --grep commit"})
    add_event(store, "PostToolUse", "p1", tool_name="Write", tool_input={"file_path": "/a"}, duration_ms=2000)
    add_event(store, "PermissionDenied", "p1", tool_name="Bash")
    add_event(store, "PreCompact", "p1")
    add_event(store, "SubagentStart", "p1", agent_type="Explore")
    add_event(store, "user_prompt", "p1", source="claude-code-otel", command_name="effort")
    for source, cost in (("repl_main_thread", 1.0), ("agent:builtin:Explore", 0.5), ("compact", 0.25)):
        add_event(store, "api_request", "p1", source="claude-code-otel", model="opus", cost_usd=cost, query_source=source, duration_ms=500)
    index.ensure()
    row = indexed(store, id)
    assert row["title"] == "Docs" and row["slash_command"] == "/effort"  # the telemetry command name, with its slash
    # Code in the injected selection is not what you typed; the URL and the mention are.
    assert (row["code_blocks"], row["urls"], row["mentions"]) == (0, 1, 1)
    assert (row["git_commits"], row["subagent_tool_calls"], row["subagents"], row["files_written"]) == (1, 1, 1, 1)
    assert (row["denials"], row["compactions"], row["tool_time_s"], row["api_time_s"]) == (1, 1, 2.0, 1.5)
    assert row["text_hash"] == indexer.text_hash("see HTTPS://a.b   and @x.py")
    work = store.db.execute("select work, query_source, cost_usd from prompt_index_work where id = ? order by cost_usd desc", (id,))
    assert [tuple(r) for r in work] == [
        ("main", "repl_main_thread", 1.0),
        ("subagent", "agent:builtin:Explore", 0.5),
        ("system", "compact", 0.25),
    ]


def test_session_telemetry_is_read_incrementally(store, index):
    add_prompt(store, "hello", "p1")
    for value in (3, 4):
        row = store.add_event(
            "claude-code-otel", {"event": "claude_code.lines_of_code.count", "session_id": "s1"}, {"type": "added", "value": value}
        )
    index.ensure()
    totals = store.db.execute("select name, total(value) from session_metric").fetchone()
    assert tuple(totals) == ("claude_code.lines_of_code.count[added]", 7.0)
    store.add_event(
        "claude-code-otel", {"event": "user_prompt", "session_id": "s1"}, {"command_name": "model", "command_source": "builtin"}
    )
    index.ensure()
    assert [tuple(r) for r in store.db.execute("select command, source from session_command")] == [("/model", "builtin")]
    assert store.index.telemetry_after > row["id"]
    index.rebuild()  # rebuilt from the raw events, nothing counted twice
    assert store.db.execute("select total(value) from session_metric").fetchone()[0] == 7.0
