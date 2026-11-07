"""Field catalog: every field seen in stored payloads, next to the fields each tool documents, so
features can be chosen from real data."""

from collections.abc import Iterable
from typing import Any

from pydantic import BaseModel

from gitprompt.core.payload import EMPTY, flatten
from gitprompt.insights.derivations import DERIVED
from gitprompt.sources.spec import Spec

SAMPLE_CHARS = 200
ANY_EVENT = "*"  # in a spec's `documented`: fields every event of the source carries


class Field(BaseModel):
    source: str
    event: str
    path: str
    count: int = 0
    sample: Any = None
    documented: bool = False


class NotCaptured(BaseModel):
    source: str
    event: str
    reason: str


class Catalog(BaseModel):
    fields: list[Field]
    not_captured: list[NotCaptured]
    derived: dict[str, str]  # GitPrompt-derived value -> what it means


Key = tuple[str, str, str]  # (source, event, path)


def _seen(samples: Iterable[tuple[str, str, dict]]) -> dict[Key, Field]:
    seen: dict[Key, Field] = {}
    for source, event, data in samples:
        for path, value in flatten(data):
            field = seen.setdefault((source, event, path), Field(source=source, event=event, path=path))
            field.count += 1
            if value not in EMPTY:
                field.sample = value[:SAMPLE_CHARS] if isinstance(value, str) else value
    return seen


def _covers(name: str, path: str) -> bool:
    """A documented field covers itself and everything nested under it."""
    return path == name or path.startswith((f"{name}.", f"{name}["))


def _mark_documented(seen: dict[Key, Field], spec: Spec) -> None:
    for event, names in spec.documented.items():
        for name in names:
            matches = [f for (s, e, p), f in seen.items() if s == spec.id and event in (ANY_EVENT, e) and _covers(name, p)]
            for f in matches:
                f.documented = True
            if not matches:  # documented but not seen yet
                seen[(spec.id, event, name)] = Field(source=spec.id, event=event, path=name, documented=True)


def build_catalog(samples: Iterable[tuple[str, str, dict]], specs: Iterable[Spec]) -> Catalog:
    specs = list(specs)
    seen = _seen(samples)
    for spec in specs:
        _mark_documented(seen, spec)
    return Catalog(
        fields=sorted(seen.values(), key=lambda f: (f.source, f.event, f.path)),
        not_captured=[NotCaptured(source=s.id, event=e, reason=r) for s in specs for e, r in (s.not_captured | s.drop_events).items()],
        derived=DERIVED,
    )
