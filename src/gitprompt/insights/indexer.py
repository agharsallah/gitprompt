"""Keeps the prompt index (storage/index.py) in step with the raw rows, using the same derivations
the prompt API returns (derivations.derive), so a value is computed in one place only (ADR-0012).

    ensure()      before every read: rebuild if the rules version changed, else index new prompts
                  and the turns ingest marked as changed (a Stop or api_request changes a prompt)
    reindex(ids)  after an edit of some prompts (hiding a secret)
    rebuild()     everything, e.g. after a remap, a compaction or `gitprompt reindex`

The rules version hashes what the values depend on besides the raw rows: the source code of the
modules that compute them, each spec's `origin` rules and `injected` tags, and the local timezone
(day and hour buckets). Changing any of them rebuilds the index on next use, with no migration.
"""

import hashlib
import sys
import time
from collections import defaultdict
from collections.abc import Iterable, Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from gitprompt.insights import derivations, origin
from gitprompt.sources.spec import Spec
from gitprompt.storage import index as storage_index
from gitprompt.storage.index import USAGE, Entry
from gitprompt.storage.store import Store

BATCH = 500  # prompts derived per query batch during a rebuild


def _code_version() -> str:
    """A hash of the modules that compute or store the indexed values (this one included)."""
    digest = hashlib.sha256()
    for module in (derivations, origin, storage_index, sys.modules[__name__]):
        digest.update(Path(module.__file__ or "").read_bytes())
    return digest.hexdigest()[:16]


CODE_VERSION = _code_version()


def rules_version(specs: Mapping[str, Spec]) -> str:
    rules = [(s.id, [r.model_dump(mode="json") for r in s.origin], s.injected) for s in sorted(specs.values(), key=lambda s: s.id)]
    data = repr((CODE_VERSION, rules, time.tzname))
    return hashlib.sha256(data.encode()).hexdigest()[:16]


def local_time(received_at: str) -> tuple[str, int, int]:
    """(day, hour, ISO weekday) in this machine's local timezone, with its DST rules at that instant."""
    t = datetime.fromisoformat(received_at).astimezone()
    return t.date().isoformat(), t.hour, t.isoweekday()


def entry(derived: dict[str, Any]) -> Entry:
    """The index row of a prompt, projected from derivations.derive()."""
    fields, context, stats, summary = derived["fields"], derived["context"], derived["stats"], derived["summary"]
    day, hour, weekday = local_time(derived["received_at"])
    typed = derived["typed_text"]
    row = {
        "id": derived["id"],
        "prompt_id": fields.get("prompt_id"),
        "source": derived["source"],
        "session": fields.get("session_id"),
        "repo": context.get("git_root") or fields.get("cwd"),
        "received_at": derived["received_at"],
        "day": day,
        "hour": hour,
        "weekday": weekday,
        "origin": derived["origin"],
        "typed_text": typed,
        "chars": len(typed or ""),
        "slash_command": summary.get("slash_command") or stats.get("slash_command"),
        **{k: summary.get(k, 0) for k in USAGE},
        "response_s": summary.get("response_s"),
        "ended_by": summary.get("ended_by"),
        "tool_calls": sum(summary.get("tool_calls", {}).values()),
        "tool_failures": summary.get("tool_failures", 0),
    }
    models = [{"model": model, **{k: usage.get(k, 0) for k in USAGE}} for model, usage in summary.get("model_usage", {}).items()]
    tools = [(name, derivations.tool_kind(name), n) for name, n in summary.get("tool_calls", {}).items()]
    return Entry(row, models, tools)


class Indexer:
    def __init__(self, store: Store, specs: Mapping[str, Spec]) -> None:
        self.store, self.specs = store, specs
        self.version = rules_version(specs)

    def entries(self, ids: list[int]) -> list[Entry]:
        rows = self.store.prompts(ids)
        events: defaultdict[str, list[dict]] = defaultdict(list)
        for e in self.store.events_for_prompts(sorted({pid for r in rows if (pid := r["fields"].get("prompt_id"))})):
            events[e["prompt_id"]].append(e)
        return [entry(derivations.derive(r, events[r["fields"].get("prompt_id")], self.specs.get(r["source"]))) for r in rows]

    def reindex(self, ids: Iterable[int]) -> int:
        with self.store.transaction():
            return self._reindex(ids)

    def _reindex(self, ids: Iterable[int]) -> int:
        ids = sorted(set(ids))
        for start in range(0, len(ids), BATCH):
            self.store.index.upsert(self.entries(ids[start : start + BATCH]))
        return len(ids)

    def rebuild(self) -> int:
        """Drop the index and compute it again from the raw rows."""
        index = self.store.index
        with self.store.transaction():
            index.drop()
            index.create()
            index.version = self.version
            return self._reindex(self.store.prompt_ids())

    def ensure(self) -> int:
        """Bring the index up to date; cheap when nothing changed, so it runs before every read."""
        index = self.store.index
        if index.version != self.version:
            return self.rebuild()
        with self.store.transaction():
            return self._reindex([*index.unindexed(), *self.store.prompts_of_turns(index.take_marked())])
