"""Field coverage: every key a connector's fixture payloads carry is used, or declared unused with a
reason (ADR-0021). A tool update that adds a field then fails here until someone decides what it
is for, instead of the field being stored and never shown.

A payload key (a dotted path, list items as `[]`) is covered when it is

    mapped     a candidate path of the spec's `map`, its `usage` values or an origin rule's `path`
    derived    read by the derivations: the turn and session summaries, origin, the prompt's facts
               (found by replaying the fixtures and recording every key they read)
    shown      in the spec's `shown`: the dashboard reads it from the raw payload itself
    ignored    in the spec's `ignored`, with the reason it is not used

A key covers what is under it, except that a derivation which reads into a key (`token_usage` ->
`token_usage.input_tokens`) covers only what it read.
"""

import json
from typing import Any

import pytest
from test_conformance import TOOLS, cases, replay  # the fixtures and how they are replayed

from gitprompt.capture import pipeline
from gitprompt.core.payload import flatten
from gitprompt.insights import derivations, origin
from gitprompt.insights.indexer import prompt_facts
from gitprompt.sources.spec import Spec, load_specs
from gitprompt.storage.store import Store


class Traced(dict):
    """A payload that records the paths of the keys read from it (nested objects too)."""

    def __init__(self, data: dict, seen: set[str], path: str = "") -> None:
        super().__init__(data)
        self.seen, self.path = seen, path

    def _read(self, key: Any, value: Any) -> Any:
        path = f"{self.path}.{key}" if self.path else str(key)
        self.seen.add(path)
        return trace(value, self.seen, path)

    def __getitem__(self, key: Any) -> Any:
        return self._read(key, super().__getitem__(key))

    def get(self, key: Any, default: Any = None) -> Any:
        return self._read(key, super().__getitem__(key)) if key in self else default

    def values(self):  # type: ignore[override]  # reading every value reads every key
        return [self[k] for k in self.keys()]

    def items(self):  # type: ignore[override]
        return [(k, self[k]) for k in self.keys()]


def trace(value: Any, seen: set[str], path: str) -> Any:
    if isinstance(value, dict):
        return Traced(value, seen, path)
    if isinstance(value, list):
        return [trace(v, seen, f"{path}[]") for v in value]
    return value


def _normal(path: str) -> str:
    """`workspace_roots[0]` and `workspace_roots[]` name the same key."""
    return path.replace("[0]", "[]")


def _under(path: str, key: str) -> bool:
    return path == key or path.startswith((f"{key}.", f"{key}["))


def declared(spec: Spec) -> set[str]:
    """Keys covered by the spec's data: mapped, model-call values, origin rule paths, shown, ignored."""
    paths = {p for candidates in spec.map.values() for p in candidates}
    paths |= set(spec.usage.values.values())
    paths |= {r.when.path for r in spec.origin if r.when.path}
    return {_normal(p) for p in paths | spec.shown.keys() | spec.ignored.keys()}


def derived_reads(spec: Spec, settings, monkeypatch) -> set[str]:
    """Every payload key the derivations read when the fixtures are replayed."""
    monkeypatch.setattr(pipeline, "observe", lambda spec, cwd: {})
    store = Store(settings.db_path)
    for _, case in cases(spec.id):
        replay(spec, store, case["payload"])
    seen: set[str] = set()
    events = [_row(e) for e in store.db.execute("select * from event order by id")]
    for e in events:
        e["raw"] = trace(e["raw"], seen, "")
    for row in store.db.execute("select * from prompt order by id").fetchall():
        prompt = _row(row) | {"turn": 1, "prev_at": None}
        prompt["raw"] = trace(prompt["raw"], seen, "")
        turn = [e for e in events if e["prompt_id"] and e["prompt_id"] == prompt["fields"].get("prompt_id")]
        derived = derivations.derive(prompt, turn, spec)
        prompt_facts(derived, spec, None)
        origin.classify(prompt, turn, spec)
    for session in {e["session_id"] for e in events if e["session_id"]}:
        derivations.session([e for e in events if e["session_id"] == session])
    return {_normal(p) for p in seen}


def _row(row) -> dict[str, Any]:
    return {k: json.loads(row[k]) if k in ("fields", "raw", "context") else row[k] for k in row.keys()}  # noqa: SIM118


def uncovered(payload: dict, static: set[str], reads: set[str]) -> list[str]:
    def covered(path: str) -> bool:
        if any(_under(path, key) for key in static):
            return True
        # A key read as a whole covers what is under it; one read into covers only what was read.
        return path in reads or any(_under(path, key) and not any(r != key and _under(r, key) for r in reads) for key in reads)

    return sorted({p for p, _ in flatten(payload) if p and not covered(_normal(p))})


@pytest.mark.parametrize("tool", TOOLS)
def test_every_fixture_key_is_used_or_ignored_with_a_reason(tool, settings, monkeypatch):
    spec = load_specs(settings)[tool]
    static, reads = declared(spec), derived_reads(spec, settings, monkeypatch)
    missing = {name: keys for name, case in cases(tool) if (keys := uncovered(case["payload"], static, reads))}
    assert not missing, (
        f"{tool}: payload keys neither mapped, derived, shown nor ignored: {missing}. Map or show them, "
        f"or add them to `ignored` in {tool}.yaml with the reason."
    )


def test_an_unknown_key_fails_coverage(settings, monkeypatch):
    """The check itself: a key a tool update adds is reported until it is used or ignored."""
    spec = load_specs(settings)["codex"]
    static, reads = declared(spec), derived_reads(spec, settings, monkeypatch)
    payload = {**cases("codex")[1][1]["payload"], "brand_new_field": 1, "token_usage": {"input_tokens": 1, "new_kind": 2}}
    assert uncovered(payload, static, reads) == ["brand_new_field", "token_usage.new_kind"]


def test_ignored_keys_have_a_reason(settings):
    for spec in load_specs(settings).values():
        assert all(reason.strip() for reason in spec.ignored.values()), spec.id
        assert all(where.strip() for where in spec.shown.values()), spec.id
