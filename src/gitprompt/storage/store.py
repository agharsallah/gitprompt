"""SQLite storage. Raw payloads are always kept, so mapped fields can be rebuilt; everything else
GitPrompt shows about a prompt is derived on read (see insights/derivations.py), so it never needs a migration."""

import json
import sqlite3
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

SCHEMA = """
create table if not exists prompt (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  prompt      text,                -- null = the mapping found no prompt (drift signal)
  fields      text not null,       -- JSON: values extracted from raw by the source spec
  context     text not null,       -- JSON: observed by GitPrompt at ingest (git state, tool version)
  raw         text not null        -- JSON: the payload exactly as received
);
create unique index if not exists prompt_dedup on prompt (source, json_extract(fields, '$.prompt_id'));
create index if not exists prompt_recent on prompt (received_at desc);

-- Every other event (tool calls, turn end, telemetry...). Joined to prompts by prompt_id/session_id.
create table if not exists event (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  event       text,
  session_id  text,
  prompt_id   text,
  fields      text not null,
  raw         text not null        -- long strings are clipped (see core/payload.py clip)
);
create index if not exists event_prompt on event (prompt_id);
create index if not exists event_session on event (session_id);

-- The user's own notes on a prompt: {starred, rating, tags, note}.
create table if not exists annotation (id integer primary key, data text not null);

drop view if exists prompt_view;
create view prompt_view as
select p.*, coalesce(a.data, '{}') annotation,
       row_number() over w turn,
       lag(p.received_at) over w prev_at,
       lead(json_extract(p.context, '$.git_head')) over w next_head
from prompt p left join annotation a on a.id = p.id
window w as (partition by p.source, json_extract(p.fields, '$.session_id') order by p.id);
"""

JSON_COLUMNS = ("fields", "context", "raw", "annotation")

# Prompt list filters: query parameter -> SQL condition on prompt_view. To add a filter, add a line
# here and the matching query parameter in api/routes/prompts.py.
FILTERS = {
    "q": "prompt like '%' || ? || '%'",
    "source": "source = ?",
    "before": "id < ?",
    "session": "json_extract(fields, '$.session_id') = ?",
    "repo": "json_extract(context, '$.git_root') = ?",
    "tag": "exists (select 1 from json_each(annotation, '$.tags') where value = ?)",
    "starred": "json_extract(annotation, '$.starred') = ?",
}


def now() -> str:
    return datetime.now(UTC).isoformat()


class Store:
    def __init__(self, path: Path):
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("pragma journal_mode = wal")
        self.db.executescript(SCHEMA)
        path.chmod(0o600)

    def close(self) -> None:
        self.db.close()

    @contextmanager
    def transaction(self) -> Iterator[None]:
        """All-or-nothing for multi-row updates (the connection is otherwise in autocommit mode)."""
        self.db.execute("begin")
        try:
            yield
        except BaseException:
            self.db.execute("rollback")
            raise
        self.db.execute("commit")

    # --- writes from the receiver ---

    def add_prompt(self, source: str, fields: dict, context: dict, raw: dict) -> dict | None:
        """Insert a prompt; None if this source already sent the same prompt_id."""
        row = self.db.execute(
            "insert into prompt (source, received_at, prompt, fields, context, raw) values (?, ?, ?, ?, ?, ?)"
            " on conflict do nothing returning *",
            (source, now(), _text(fields), _dump(fields), _dump(context), _dump(raw)),
        ).fetchone()
        return _row(row) if row else None

    def add_event(self, source: str, fields: dict, raw: dict) -> dict:
        row = self.db.execute(
            "insert into event (source, received_at, event, session_id, prompt_id, fields, raw) values (?, ?, ?, ?, ?, ?, ?) returning *",
            (source, now(), *_event_keys(fields), _dump(fields), _dump(raw)),
        ).fetchone()
        return _row(row)

    # --- reads ---

    def search(self, limit: int = 50, **filters: Any) -> list[dict]:
        """Newest prompts first, narrowed by FILTERS (empty values are ignored)."""
        if unknown := filters.keys() - FILTERS.keys():
            raise ValueError(f"unknown prompt filters: {sorted(unknown)}")
        active = {k: v for k, v in filters.items() if v not in (None, "", False)}
        where = " and ".join(["1=1", *(FILTERS[k] for k in active)])
        rows = self.db.execute(f"select * from prompt_view where {where} order by id desc limit ?", [*active.values(), limit])
        return [_row(r) for r in rows]

    def get(self, id: int) -> dict | None:
        row = self.db.execute("select * from prompt_view where id = ?", (id,)).fetchone()
        return _row(row) if row else None

    def events_for_prompts(self, prompt_ids: list[str]) -> list[dict]:
        return self._events("prompt_id", prompt_ids)

    def events_for_session(self, session_id: str) -> list[dict]:
        return self._events("session_id", [session_id])

    def _events(self, column: Literal["prompt_id", "session_id"], values: list[str]) -> list[dict]:
        if not values:
            return []
        marks = ",".join("?" * len(values))
        return [_row(r) for r in self.db.execute(f"select * from event where {column} in ({marks}) order by id", values)]

    def samples(self, limit: int = 5000) -> list[tuple[str, str, dict]]:
        """(source, event, data) for the most recent payloads, for the field catalog."""
        prompts = self.db.execute(
            "select source, json_extract(fields, '$.event') e, raw, context from prompt order by id desc limit ?", (limit,)
        )
        events = self.db.execute("select source, event e, raw from event order by id desc limit ?", (limit,))
        rows = [(r["source"], r["e"] or "?", json.loads(r["raw"])) for r in events]
        for r in prompts:
            rows += [(r["source"], r["e"] or "?", json.loads(r["raw"])), ("gitprompt", "context (at ingest)", json.loads(r["context"]))]
        return rows

    def stats(self) -> dict[str, dict]:
        """Per source: prompt total, prompts with no text (drift signal), event count, last activity."""
        prompts = self.db.execute(
            "select source, count(*) total, sum(prompt is null) unmapped, max(received_at) last_at from prompt group by source"
        )
        out = {r["source"]: dict(r) for r in prompts}
        for r in self.db.execute("select source, count(*) events, max(received_at) last_at from event group by source"):
            out[r["source"]] = {"last_at": r["last_at"], **out.get(r["source"], {}), "events": r["events"]}
        return out

    # --- edits ---

    def annotate(self, id: int, data: dict) -> None:
        self.db.execute("insert or replace into annotation (id, data) values (?, ?)", (id, _dump(data)))

    def rewrite(self, id: int, edit: Callable[[Any], Any]) -> int:
        """Apply `edit` to a prompt's stored JSON and to its events (e.g. to hide a secret).
        Returns the number of rows changed."""
        row = self.get(id)
        if not row:
            return 0
        pid = row["fields"].get("prompt_id")
        events = self.events_for_prompts([pid]) if pid else []
        with self.transaction():
            fields = edit(row["fields"])
            self.db.execute(
                "update prompt set prompt = ?, fields = ?, context = ?, raw = ? where id = ?",
                (_text(fields), _dump(fields), _dump(edit(row["context"])), _dump(edit(row["raw"])), id),
            )
            for e in events:
                self.db.execute(
                    "update event set fields = ?, raw = ? where id = ?", (_dump(edit(e["fields"])), _dump(edit(e["raw"])), e["id"])
                )
        return 1 + len(events)

    def remap(self, source: str, mapper: Callable[[dict], dict]) -> int:
        """Re-run `mapper(raw) -> fields` over every stored payload of a source (after a spec fix)."""
        with self.transaction():
            prompts = self.db.execute("select id, raw from prompt where source = ?", (source,)).fetchall()
            for r in prompts:
                fields = mapper(json.loads(r["raw"]))
                self.db.execute("update prompt set prompt = ?, fields = ? where id = ?", (_text(fields), _dump(fields), r["id"]))
            events = self.db.execute("select id, raw from event where source = ?", (source,)).fetchall()
            for r in events:
                fields = mapper(json.loads(r["raw"]))
                self.db.execute(
                    "update event set event = ?, session_id = ?, prompt_id = ?, fields = ? where id = ?",
                    (*_event_keys(fields), _dump(fields), r["id"]),
                )
        return len(prompts) + len(events)


def _text(fields: dict) -> str | None:
    return fields["prompt"] if isinstance(fields.get("prompt"), str) else None


def _event_keys(fields: dict) -> tuple[Any, Any, Any]:
    """The mapped fields that get their own (indexed) event columns."""
    return fields.get("event"), fields.get("session_id"), fields.get("prompt_id")


def _dump(value: Any) -> str:
    return json.dumps(value)


def _row(row: sqlite3.Row) -> dict[str, Any]:
    return {k: json.loads(row[k]) if k in JSON_COLUMNS else row[k] for k in row.keys()}  # noqa: SIM118 (Row iterates values)
