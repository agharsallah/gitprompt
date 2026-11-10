"""Storage hygiene: noise events are not stored, already-stored noise is compacted, retention keeps prompts."""

import asyncio
from datetime import UTC, datetime, timedelta

import pytest

from gitprompt.capture import compact, pipeline
from gitprompt.insights.indexer import Indexer
from gitprompt.sources.spec import load_specs
from gitprompt.storage.store import Store

OTEL = "claude-code-otel"


def otlp(*names: str) -> dict:
    """An OTLP/HTTP JSON logs batch with one record per event name (synthetic values)."""
    records = [
        {
            "body": {"stringValue": f"claude_code.{name}"},
            "attributes": [
                {"key": "event.name", "value": {"stringValue": name}},
                {"key": "session.id", "value": {"stringValue": "s1"}},
                {"key": "prompt.id", "value": {"stringValue": "p1"}},
            ],
        }
        for name in names
    ]
    return {"resourceLogs": [{"resource": {"attributes": []}, "scopeLogs": [{"logRecords": records}]}]}


@pytest.fixture
def store(settings):
    store = Store(settings.db_path)
    yield store
    store.close()


@pytest.fixture
def specs(settings):
    return load_specs(settings)


def stored(store: Store) -> list[str]:
    return [r["event"] for r in store.db.execute("select event from event order by id")]


def add(store: Store, source: str, event: str, days_ago: int = 0) -> None:
    row = store.add_event(source, {"event": event, "session_id": "s1"}, {"event.name": event})
    received = (datetime.now(UTC) - timedelta(days=days_ago)).isoformat()
    store.db.execute("update event set received_at = ? where id = ?", (received, row["id"]))


def test_ingest_skips_dropped_events(store, specs):
    body = otlp("hook_registered", "hook_execution_start", "hook_execution_complete", "api_request", "plugin_loaded")
    captured = asyncio.run(pipeline.capture(specs[OTEL], store, body))
    assert captured.events == 1 and stored(store) == ["api_request"]


def test_dropped_events_are_listed_on_the_fields_page(client):
    not_captured = {(n["source"], n["event"]): n["reason"] for n in client.get("/api/fields").json()["not_captured"]}
    assert not_captured[(OTEL, "hook_execution_*")].startswith("Not stored")


def test_compact_is_a_dry_run_unless_applied(store, specs):
    for event in ("hook_registered", "hook_registered", "hook_execution_start", "api_request"):
        add(store, OTEL, event)
    add(store, "unknown-source", "hook_registered")  # no spec: never touched

    plan = compact.compact(store, specs)
    assert {(r.event, r.rows) for r in plan.removals} == {("hook_registered", 2), ("hook_execution_start", 1)}
    assert plan.bytes > 0 and plan.size_after is None and len(stored(store)) == 5

    done = compact.compact(store, specs, apply=True)
    assert done.rows == 3 and done.size_after is not None
    assert stored(store) == ["api_request", "hook_registered"]
    assert compact.compact(store, specs).removals == []


def test_retention_removes_old_events_only_never_prompts_or_annotations(store, specs, hook_payload):
    prompt = store.add_prompt("claude-code", hook_payload, {}, hook_payload)
    store.annotate(prompt["id"], {"starred": True})
    store.db.execute("update prompt set received_at = '2000-01-01T00:00:00+00:00'")
    add(store, "claude-code", "Stop", days_ago=40)
    add(store, OTEL, "hook_registered", days_ago=40)  # noise and old: counted once, as noise
    add(store, "claude-code", "PostToolUse", days_ago=2)

    plan = compact.compact(store, specs, older_than_days=30)
    assert {(r.event, r.why) for r in plan.removals} == {("hook_registered", "dropped by spec"), ("Stop", "expired")}

    compact.compact(store, specs, older_than_days=30, apply=True)
    assert stored(store) == ["PostToolUse"]
    assert store.get(prompt["id"])["annotation"] == {"starred": True}


def test_retention_needs_at_least_one_day(store):
    with pytest.raises(ValueError):
        compact.expired(store, 0)


def test_deleting_events_rebuilds_the_prompt_index_on_next_read(store, specs):

    indexer = Indexer(store, specs)
    indexer.ensure()
    add(store, "claude-code", "Stop", days_ago=40)
    assert compact.remove(store, compact.expired(store, 30)) == 1
    assert store.index.version is None  # stale: retention, Settings › Data and compact all go through remove()
    indexer.ensure()
    assert store.index.version == indexer.version
    assert compact.remove(store, []) == 0 and store.index.version == indexer.version  # nothing deleted: kept
