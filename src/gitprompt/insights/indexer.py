"""Keeps the prompt index (storage/index.py) in step with the raw rows, using the same derivations
the prompt API returns (derivations.derive), so a value is computed in one place only (ADR-0012).

    ensure()      before every read: rebuild if the rules version changed, else index new prompts
                  and the turns ingest marked as changed (a Stop or a model call changes a prompt)
    reindex(ids)  after an edit of some prompts (hiding a secret)
    rebuild()     everything, e.g. after a remap, a compaction or `gitprompt reindex`

The rules version hashes what the values depend on besides the raw rows: the source code of the
modules that compute them, each spec's `origin` rules, `injected` tags, `tool_kinds`, `tool_files` and
`usage`, and the local timezone (day and hour buckets). Changing any of them rebuilds the index on next use, with no migration.
"""

import hashlib
import sys
import time
from collections import defaultdict
from collections.abc import Iterable, Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from gitprompt.insights import derivations, facts, origin
from gitprompt.sources.spec import Spec
from gitprompt.storage import index as storage_index
from gitprompt.storage.index import USAGE, Entry
from gitprompt.storage.store import Store

BATCH = 500  # prompts derived per query batch during a rebuild


def _code_version() -> str:
    """A hash of the modules that compute or store the indexed values (this one included)."""
    digest = hashlib.sha256()
    for module in (derivations, facts, origin, storage_index, sys.modules[__name__]):
        digest.update(Path(module.__file__ or "").read_bytes())
    return digest.hexdigest()[:16]


CODE_VERSION = _code_version()


def rules_version(specs: Mapping[str, Spec]) -> str:
    rules = [
        (
            s.id,
            [r.model_dump(mode="json") for r in s.origin],
            s.injected,
            s.tool_kinds,
            {k: p.pattern for k, p in s.tool_files.items()},
            s.usage.model_dump(),
        )
        for s in sorted(specs.values(), key=lambda s: s.id)
    ]
    data = repr((CODE_VERSION, rules, time.tzname))
    return hashlib.sha256(data.encode()).hexdigest()[:16]


def local_time(received_at: str) -> tuple[str, int, int]:
    """(day, hour, ISO weekday) in this machine's local timezone, with its DST rules at that instant."""
    t = datetime.fromisoformat(received_at).astimezone()
    return t.date().isoformat(), t.hour, t.isoweekday()


def text_hash(typed: str | None) -> str | None:
    """Equal for texts that differ only in case and spacing: the same prompt sent again."""
    words = " ".join((typed or "").lower().split())
    return hashlib.sha256(words.encode()).hexdigest()[:16] if words else None


def slash(command: str | None) -> str | None:
    """Slash commands with their leading slash, whichever event named them."""
    return command and "/" + command.lstrip("/")


def entry(derived: dict[str, Any]) -> Entry:
    """The index row of a prompt, projected from derivations.derive()."""
    fields, context, stats, summary = derived["fields"], derived["context"], derived["stats"], derived["summary"]
    day, hour, weekday = local_time(derived["received_at"])
    typed = derived["typed_text"]
    # What the user wrote is counted in the typed text; secrets in the prompt as received (context too).
    typed_stats = derivations.prompt_stats(typed or "")
    row = {
        "id": derived["id"],
        "prompt_id": fields.get("prompt_id"),
        "source": derived["source"],
        "session": fields.get("session_id"),
        "repo": context.get("git_root") or fields.get("cwd"),
        "repo_name": context.get("git_repo"),
        "branch": context.get("git_branch"),
        "title": fields.get("session_title"),
        "received_at": derived["received_at"],
        "day": day,
        "hour": hour,
        "weekday": weekday,
        "origin": derived["origin"],
        "typed_text": typed,
        "chars": len(typed or ""),
        "text_hash": text_hash(typed),
        "slash_command": slash(summary.get("slash_command") or stats.get("slash_command")),
        "code_blocks": typed_stats.get("code_blocks", 0),
        "urls": len(typed_stats.get("urls", [])),
        "mentions": len(typed_stats.get("at_mentions", [])),
        "secrets": len(stats.get("secrets", [])),
        **{k: summary.get(k, 0) for k in USAGE},
        "api_time_s": summary.get("api_time_s", 0),
        "api_errors": summary.get("api_errors", 0),
        "response_s": summary.get("response_s"),
        "ended_by": summary.get("ended_by"),
        "tool_calls": sum(summary.get("tool_calls", {}).values()),
        "tool_failures": summary.get("tool_failures", 0),
        "tool_time_s": summary.get("tool_time_s", 0),
        "subagent_tool_calls": summary.get("subagent_tool_calls", 0),
        "subagents": sum(summary.get("subagents", {}).values()),
        "files_written": len(summary.get("files_written", [])),
        "git_commits": summary.get("git_commits", 0),
        "compactions": summary.get("compactions", 0),
        "denials": len(summary.get("permission_denied", [])),
        "rejections": summary.get("tool_rejections", 0),
        "model": facts.main_model(summary, fields),
        "permission_mode": fields.get("permission_mode"),
        "tool_version": context.get("tool_version"),
        "reasoning_tokens": summary.get("reasoning_tokens", 0),
        "answered": int(bool(summary.get("response"))),
    }
    # Models with usage come from model calls; without them, the model the tool reported with the
    # prompt (hooks that carry one) still counts the prompt for that model, with no usage.
    model_usage = summary.get("model_usage") or ({fields["model"]: {}} if fields.get("model") else {})
    models = [{"model": model, **{k: usage.get(k, 0) for k in USAGE}} for model, usage in model_usage.items()]
    kinds = summary.get("tool_kind_of", {})
    tools = [(name, kinds.get(name, "other"), n) for name, n in summary.get("tool_calls", {}).items()]
    work = [
        {"work": derivations.work_kind(source), "query_source": source, **{k: usage.get(k, 0) for k in USAGE}}
        for source, usage in summary.get("work_usage", {}).items()
    ]
    return Entry(row, models, tools, work)


def prompt_facts(derived: dict[str, Any], spec: Spec | None, outcome: str | None) -> dict[str, Any]:
    """A derived prompt's facts (insights/facts.py), from the same projection the index stores."""
    projected = entry(derived)
    return facts.facts(projected.row, spec, outcome, [m["model"] for m in projected.models if m["model"]])


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

    def _index_telemetry(self) -> None:
        """Telemetry metrics and slash commands that arrived since the last read (by event id)."""
        index = self.store.index
        events = self.store.events_after(index.telemetry_after, derivations.METRIC_PREFIX, (derivations.COMMAND_EVENT,))
        if not events:
            return
        where = [(e["id"], e["session_id"], e["received_at"]) for e in events]
        metrics = [(*w, *m) for w, e in zip(where, events, strict=True) if (m := derivations.metric(e))]
        commands = [(*w, *c) for w, e in zip(where, events, strict=True) if (c := derivations.command(e))]
        index.add_telemetry(events[-1]["id"], metrics, commands)

    def rebuild(self) -> int:
        """Drop the index and compute it again from the raw rows."""
        index = self.store.index
        with self.store.transaction():
            index.drop()
            index.create()
            index.version = self.version
            self._index_telemetry()
            return self._reindex(self.store.prompt_ids())

    def ensure(self) -> int:
        """Bring the index up to date; cheap when nothing changed, so it runs before every read."""
        index = self.store.index
        if index.version != self.version:
            return self.rebuild()
        with self.store.transaction():
            self._index_telemetry()
            return self._reindex([*index.unindexed(), *self.store.prompts_of_turns(index.take_marked())])
