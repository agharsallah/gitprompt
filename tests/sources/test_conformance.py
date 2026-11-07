"""Connector conformance kit: a connector is "done" when its fixtures pass.

`fixtures/<spec id>/*.json` are synthetic payloads built from the tool's documented examples (never
real data), replayed in file-name order through the capture pipeline, the way the tool would send
them. Each file holds:

    about     what the case shows
    payload   the hook payload, as the tool documents it
    expect    stored_as   "prompt", "event" or "dropped"
              fields      mapped fields that must have these values (a subset)
              origin      prompts: who produced the text (insights/origin.py)
              typed_text  prompts, optional: the text with injected context removed
              turn        events: the fixture of the prompt it joins (by prompt_id or by the
                          spec's `join` fallback), or null for none

To add a connector: its YAML spec, then a folder here named after its id.
"""

import asyncio
import json
from pathlib import Path

import pytest

from gitprompt.capture import pipeline
from gitprompt.insights.origin import classify
from gitprompt.sources.spec import load_specs
from gitprompt.storage.store import Store

FIXTURES = Path(__file__).parent / "fixtures"
TOOLS = sorted(p.name for p in FIXTURES.iterdir() if p.is_dir())


def cases(tool: str) -> list[tuple[str, dict]]:
    return [(p.stem, json.loads(p.read_text())) for p in sorted((FIXTURES / tool).glob("*.json"))]


def last_id(store: Store, table: str) -> int:
    return store.db.execute(f"select coalesce(max(id), 0) from {table}").fetchone()[0]


def replay(spec, store: Store, payload: dict) -> tuple[str, dict | None]:
    """Capture one payload; which table it landed in, and the stored row."""
    before = {t: last_id(store, t) for t in ("prompt", "event")}
    asyncio.run(pipeline.capture(spec, store, payload))
    for table, previous in before.items():
        if last_id(store, table) > previous:
            row = store.db.execute(f"select * from {table} where id = ?", (last_id(store, table),)).fetchone()
            return table, {k: json.loads(row[k]) if k in ("fields", "raw", "context") else row[k] for k in row.keys()}  # noqa: SIM118
    return "dropped", None


def test_every_main_bundled_connector_has_fixtures(settings):
    mains = {spec.id for spec in load_specs(settings).values() if not spec.part_of}
    assert mains <= set(TOOLS), f"connectors without conformance fixtures: {sorted(mains - set(TOOLS))}"


@pytest.mark.parametrize("tool", TOOLS)
def test_connector_conforms(tool, settings, monkeypatch):
    monkeypatch.setattr(pipeline, "observe", lambda spec, cwd: {})  # no git or tool version on this machine
    spec = load_specs(settings).get(tool)
    assert spec, f"fixtures/{tool} has no bundled spec with that id"
    store = Store(settings.db_path)
    stored: dict[str, dict] = {}

    for name, case in cases(tool):
        expect = case["expect"]
        table, row = replay(spec, store, case["payload"])
        assert table == expect["stored_as"], f"{name}: stored as {table}"
        if row is None:
            continue
        stored[name] = row
        assert row["fields"] | expect["fields"] == row["fields"], f"{name}: mapped {row['fields']}"
        if table == "prompt":
            assert row["fields"].get("prompt_id"), f"{name}: a prompt needs a prompt id (sent, or given by `join`)"
            events = store.events_for_prompts([row["fields"]["prompt_id"]])
            derived = classify(row, events, spec)
            assert derived["origin"] == expect["origin"], f"{name}: origin {derived['origin']}"
            if "typed_text" in expect:
                assert derived["typed_text"] == expect["typed_text"], name
        else:
            turn = expect["turn"]
            want = stored[turn]["fields"]["prompt_id"] if turn else None
            assert row["prompt_id"] == want, f"{name}: joined {row['prompt_id']!r}, expected the turn of {turn}"
