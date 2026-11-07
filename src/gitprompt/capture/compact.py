"""Storage hygiene: apply today's capture rules to what was captured earlier, and optional retention.

    noise    stored events the current specs would no longer keep (`accept_events`, `drop_events`)
    expired  raw events older than N days (prompts and annotations are never removed)
    selected raw events of one source and/or received before a time (Settings > Data)

`compact` plans by default and changes nothing; with `apply=True` it deletes, then VACUUMs.
`gitprompt compact` and the Settings page are thin wrappers; capture/retention.py applies the
`event_retention_days` preference with `expired` + `remove`.
"""

from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from typing import Literal

from gitprompt.sources.spec import Spec
from gitprompt.storage.store import Store


@dataclass(frozen=True)
class Removal:
    source: str
    event: str | None
    rows: int
    bytes: int  # approximate: stored JSON of the rows
    why: Literal["dropped by spec", "expired", "selected"]
    before: str | None = None  # expired: only rows received before this time


@dataclass
class Report:
    removals: list[Removal] = field(default_factory=list)
    size_before: int = 0
    size_after: int | None = None  # set once applied

    @property
    def rows(self) -> int:
        return sum(r.rows for r in self.removals)

    @property
    def bytes(self) -> int:
        return sum(r.bytes for r in self.removals)


def cutoff(days: int, now: datetime | None = None) -> str:
    """The received_at before which an event is older than `days` days."""
    if days < 1:
        raise ValueError("retention must be at least 1 day")
    return ((now or datetime.now(UTC)) - timedelta(days=days)).isoformat()


def noise(store: Store, specs: Mapping[str, Spec]) -> list[Removal]:
    """Stored events their source's spec no longer keeps. Events of a source without a spec are kept."""
    return [
        Removal(u["source"], u["event"], u["rows"], u["bytes"], "dropped by spec")
        for u in store.event_usage()
        if (spec := specs.get(u["source"])) and not spec.keeps(u["event"])
    ]


def expired(store: Store, days: int, skip: Iterable[Removal] = ()) -> list[Removal]:
    """Raw events older than `days` days, except the kinds in `skip` (already being removed)."""
    before, skipped = cutoff(days), {(r.source, r.event) for r in skip}
    return [
        Removal(u["source"], u["event"], u["rows"], u["bytes"], "expired", before)
        for u in store.event_usage(before)
        if (u["source"], u["event"]) not in skipped
    ]


def selected(store: Store, source: str | None = None, before: str | None = None) -> list[Removal]:
    """Raw events of `source` (None = every source) received before `before` (None = any time)."""
    return [
        Removal(u["source"], u["event"], u["rows"], u["bytes"], "selected", before)
        for u in store.event_usage(before)
        if source is None or u["source"] == source
    ]


def remove(store: Store, removals: list[Removal]) -> int:
    with store.transaction():
        return sum(store.delete_events(r.source, r.event, r.before) for r in removals)


def plan(store: Store, removals: list[Removal], apply: bool = False) -> Report:
    """Report what `removals` would delete; with `apply`, delete it and VACUUM."""
    report = Report(removals, size_before=store.size())
    if apply:
        remove(store, removals)
        store.vacuum()
        report.size_after = store.size()
    return report


def compact(store: Store, specs: Mapping[str, Spec], older_than_days: int | None = None, apply: bool = False) -> Report:
    removals = noise(store, specs)
    if older_than_days is not None:
        removals += expired(store, older_than_days, skip=removals)
    return plan(store, removals, apply)
