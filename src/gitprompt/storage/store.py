"""SQLite storage. Raw payloads are always kept, so mapped fields can be rebuilt; everything else
GitPrompt shows about a prompt is derived on read (see insights/derivations.py), so it never needs a
migration. The values filters and statistics need in SQL are cached in a rebuildable index
(storage/index.py, filled by insights/indexer.py)."""

import json
import sqlite3
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

from gitprompt.storage.index import PromptIndex


def _session(alias: str = "") -> str:
    """A prompt's session key (no session = ''): the expression of the prompt_session index."""
    return f"coalesce(json_extract({alias}fields, '$.session_id'), '')"


# Prompts with their turn number and neighbours in their session; `{scope}` limits which prompts
# are numbered (Store._view numbers only the sessions it needs).
VIEW = f"""
select p.*, coalesce(a.data, '{{}}') annotation,
       row_number() over w turn,
       lag(p.received_at) over w prev_at,
       lead(json_extract(p.context, '$.git_head')) over w next_head
from prompt p left join annotation a on a.id = p.id
where {{scope}}
window w as (partition by p.source, {_session("p.")} order by p.id)"""

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
create index if not exists prompt_turn on prompt (json_extract(fields, '$.prompt_id'));

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

create index if not exists prompt_session on prompt (source, {session});
drop view if exists prompt_view;
create view prompt_view as {view};
""".replace("{session}", _session()).replace("{view}", VIEW.replace("{scope}", "1=1"))

JSON_COLUMNS = ("fields", "context", "raw", "annotation")
CHUNK = 500  # values per `in (...)` query

# Prompts as filters see them.
INDEXED = "prompt_index i"

# Prompt filters, shared by the prompt list and statistics: query parameter -> SQL condition on the
# prompt index `i` (storage/index.py), with one `?` for the value. A list value
# is passed as a JSON array (read it with json_each); a function builds a condition from the value.
# To add a filter, add a line here and the matching field in api/filters.py.
Filter = str | Callable[[Any], tuple[str, list[Any]]]

TRIGRAM = 3  # FTS5's trigram tokenizer matches words of at least 3 characters


def _words(q: str) -> tuple[str, list[Any]]:
    """Every word must appear in the typed text, anywhere, ignoring case. Words of 3+ characters go
    through the full-text index; shorter ones are checked on the rows it found (or on all rows)."""
    words = q.split()
    long, short = [w for w in words if len(w) >= TRIGRAM], [w for w in words if len(w) < TRIGRAM]
    conditions = ["i.id in (select rowid from prompt_fts where prompt_fts match ?)"] if long else []
    params: list[Any] = [" ".join('"' + w.replace('"', '""') + '"' for w in long)] if long else []
    conditions += ["i.id in (select rowid from prompt_fts where typed_text like '%' || ? || '%')"] * len(short)
    return " and ".join(conditions) or "1=1", params + short


FILTERS: dict[str, Filter] = {
    "q": _words,
    "origin": "i.origin in (select value from json_each(?))",
    "model": "i.id in (select id from prompt_index_model where model in (select value from json_each(?)))",
    "source": "i.source = ?",
    "session": "i.session = ?",
    "repo": "i.repo = ?",
    "since": "i.received_at >= ?",  # ISO-8601 UTC, see api/filters.py
    "until": "i.received_at < ?",
    "before": "i.id < ?",  # paging cursor: prompts older than this id
    "tag": "i.id in (select a.id from annotation a, json_each(a.data, '$.tags') t where t.value = ?)",
    "starred": "i.id in (select id from annotation where json_extract(data, '$.starred') = ?)",
}
EMPTY_FILTER = (None, "", False, [], ())


def where(**filters: Any) -> tuple[str, list[Any]]:
    """SQL conditions for FILTERS (empty values are ignored) and their parameters."""
    if unknown := filters.keys() - FILTERS.keys():
        raise ValueError(f"unknown prompt filters: {sorted(unknown)}")
    conditions, params = ["1=1"], []
    for key, value in filters.items():
        if value in EMPTY_FILTER:
            continue
        condition = FILTERS[key]
        if callable(condition):
            sql, values = condition(value)
        else:
            sql, values = condition, [json.dumps(list(value)) if isinstance(value, list | tuple) else value]
        conditions.append(sql)
        params += values
    return " and ".join(conditions), params


def now() -> str:
    return datetime.now(UTC).isoformat()


class Store:
    def __init__(self, path: Path):
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("pragma journal_mode = wal")
        self.db.executescript(SCHEMA)
        self.index = PromptIndex(self.db)
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
        sql, params = where(**filters)
        ids = [r[0] for r in self.db.execute(f"select i.id from {INDEXED} where {sql} order by i.id desc limit ?", [*params, limit])]
        return self._view(ids)

    def _view(self, ids: list[int]) -> list[dict]:
        """prompt_view rows (turn number, neighbours) of these prompts, in the order given. Only their
        sessions are numbered (found through the prompt_session index), not every stored prompt."""
        if not ids:
            return []
        marks = ",".join("?" * len(ids))
        scope = f"(p.source, {_session('p.')}) in (select source, {_session()} from prompt where id in ({marks}))"
        rows = self.db.execute(f"select * from ({VIEW.replace('{scope}', scope)}) where id in ({marks})", [*ids, *ids])
        by_id = {r["id"]: _row(r) for r in rows}
        return [by_id[i] for i in ids if i in by_id]

    def prompts(self, ids: list[int]) -> list[dict]:
        """Stored prompt rows (without the view's turn and neighbours), by id."""
        return self._in("select * from prompt where id", ids)

    def prompts_of_turns(self, prompt_ids: list[str]) -> list[int]:
        """Ids of the prompts these prompt_ids belong to (of any source)."""
        return [r["id"] for r in self._in("select id from prompt where json_extract(fields, '$.prompt_id')", prompt_ids)]

    def prompt_ids(self) -> list[int]:
        return [r[0] for r in self.db.execute("select id from prompt order by id")]

    def _in(self, select: str, values: list) -> list[dict]:
        out: list[dict] = []
        for start in range(0, len(values), CHUNK):
            chunk = values[start : start + CHUNK]
            out += [_row(r) for r in self.db.execute(f"{select} in ({','.join('?' * len(chunk))})", chunk)]
        return out

    def get(self, id: int) -> dict | None:
        return next(iter(self._view([id])), None)

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

    # --- housekeeping (see capture/compact.py) ---

    def event_usage(self, before: str | None = None) -> list[dict]:
        """Stored events per (source, event): rows and approximate bytes. `before`: only rows received earlier."""
        rows = self.db.execute(
            "select source, event, count(*) rows, sum(length(cast(fields as blob)) + length(cast(raw as blob))) bytes"
            " from event where received_at < coalesce(?, '9999') group by source, event order by bytes desc",
            (before,),
        )
        return [dict(r) for r in rows]

    def delete_events(self, source: str, event: str | None, before: str | None = None) -> int:
        """Delete stored events of one kind (optionally only those received before `before`). Prompts and
        annotations live in other tables and are never touched."""
        return self.db.execute(
            "delete from event where source = ? and event is ? and received_at < coalesce(?, '9999')", (source, event, before)
        ).rowcount

    def size(self) -> int:
        """Bytes the database occupies (including free pages that VACUUM would return)."""
        return self.db.execute("pragma page_count").fetchone()[0] * self.db.execute("pragma page_size").fetchone()[0]

    def vacuum(self) -> None:
        """Rewrite the file without free pages, then fold the write-ahead log back in."""
        self.db.execute("vacuum")
        self.db.execute("pragma wal_checkpoint(truncate)")

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
