"""GET /api/sources: the raw facts the dashboard derives each connector's state from."""

from datetime import UTC, datetime, timedelta

from gitprompt.api.routes.sources import HOURS, hourly
from gitprompt.storage.store import Store


def source(client, id):
    return next(s for s in client.get("/api/sources").json() if s["id"] == id)


def received(store: Store, table: str, id: int, at: datetime) -> None:
    store.db.execute(f"update {table} set received_at = ? where id = ?", (at.isoformat(), id))


def test_hourly_counts_prompts_and_events_per_hour_this_hour_last(settings):
    store = Store(settings.db_path)
    now = datetime(2026, 11, 7, 15, 40, tzinfo=UTC)
    rows = [  # in the order they arrive, as the receiver stores them
        ("event", now - timedelta(hours=24)),  # 15:40 yesterday: just outside the window
        ("event", now - timedelta(hours=23, minutes=30)),  # 16:10 yesterday: the first slot
        ("event", now - timedelta(hours=3)),  # three slots back
        ("event", now - timedelta(minutes=30)),  # this hour
        ("prompt", now - timedelta(minutes=5)),  # this hour
    ]
    for i, (table, at) in enumerate(rows):
        if table == "prompt":
            row = store.add_prompt("claude-code", {"prompt": "hi", "prompt_id": f"p{i}"}, {}, {})
        else:
            row = store.add_event("claude-code", {"event": "Stop"}, {})
        assert row
        received(store, table, row["id"], at)

    counts = hourly(store, now)["claude-code"]
    assert len(counts) == HOURS and sum(counts) == 4
    assert counts[-1] == 2 and counts[-4] == 1 and counts[0] == 1


def test_hourly_reads_only_rows_newer_than_the_window(settings):
    store = Store(settings.db_path)
    now = datetime.now(UTC)
    old = store.add_event("claude-code", {"event": "Stop"}, {})
    received(store, "event", old["id"], now - timedelta(days=3))
    store.add_event("claude-code-otel", {"event": "api_request"}, {})
    assert hourly(store, now) == {"claude-code-otel": [0] * (HOURS - 1) + [1]}
    assert hourly(Store(settings.home / "empty.db"), now) == {}


def test_sources_report_layers_activity_and_what_they_collect(client, settings, hook_payload):
    client.post("/ingest/claude-code", json=hook_payload, headers={"Authorization": f"Bearer {settings.token()}"})
    hooks, telemetry = source(client, "claude-code"), source(client, "claude-code-otel")

    assert hooks["part_of"] is None and hooks["layer"] == "Hooks" and hooks["summary"]
    assert telemetry["part_of"] == "claude-code" and telemetry["layer"] == "Telemetry"
    assert hooks["hourly"][-1] == 1 and len(hooks["hourly"]) == HOURS and sum(telemetry["hourly"]) == 0

    assert "UserPromptSubmit" in hooks["collects"] and "*" not in hooks["collects"]
    assert "PreToolUse" in hooks["not_collected"]
    assert "hook_registered" in telemetry["not_stored"] and "hook_registered" not in telemetry["collects"]
    assert "api_request" in telemetry["collects"]
