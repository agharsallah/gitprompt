"""The prompt index: a cache of derived values, so filters and statistics can use SQL `WHERE` and
`GROUP BY` over every prompt (ADR-0012). Raw rows stay the truth; the index can be dropped at any
time and is rebuilt from them (insights/indexer.py computes the values, this module only stores them).

    prompt_index        one row per prompt: origin, typed text, time buckets, usage, outcome
    prompt_index_model  requests, tokens and cost per model of a prompt (a turn can use several)
    prompt_index_tool   tool calls per tool of a prompt, with the tool's kind
    prompt_fts          FTS5 (trigram) over the typed text, rowid = prompt id
    index_dirty         turns that received events since they were indexed
    index_meta          the rules version the rows were computed with
"""

import sqlite3
from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import Any

# The order of the prompt_index columns, as Entry.row provides them. The typed text is only in
# prompt_fts: keeping it out of prompt_index keeps its rows small, so aggregates scan fewer pages.
COLUMNS = (
    "id", "prompt_id", "source", "session", "repo", "received_at", "day", "hour", "weekday",
    "origin", "chars", "slash_command",
    "api_requests", "cost_usd", "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens",
    "response_s", "ended_by", "tool_calls", "tool_failures",
)  # fmt: skip
USAGE = ("api_requests", "cost_usd", "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")

SCHEMA = """
create table if not exists index_meta (key text primary key, value text not null);

create table if not exists prompt_index (
  id            integer primary key,   -- prompt.id
  prompt_id     text,
  source        text not null,
  session       text,
  repo          text,                  -- git root at ingest, else the working directory
  received_at   text not null,         -- UTC, as stored on the prompt
  day           text not null,         -- local calendar day of the daemon's machine, YYYY-MM-DD
  hour          integer not null,      -- local hour, 0-23
  weekday       integer not null,      -- local ISO weekday, 1 = Monday
  origin        text not null,
  chars         integer not null,      -- size of the typed text (the text itself is in prompt_fts)
  slash_command text,
  api_requests  integer not null,
  cost_usd      real not null,
  input_tokens  integer not null,
  output_tokens integer not null,
  cache_read_tokens     integer not null,
  cache_creation_tokens integer not null,
  response_s    real,                  -- null while the turn has no end
  ended_by      text,                  -- Stop, StopFailure, or null
  tool_calls    integer not null,
  tool_failures integer not null
);
create index if not exists prompt_index_time on prompt_index (received_at);
create index if not exists prompt_index_origin on prompt_index (origin, received_at);
create index if not exists prompt_index_session on prompt_index (session);
create index if not exists prompt_index_repo on prompt_index (repo);

create table if not exists prompt_index_model (
  id integer not null, model text, api_requests integer not null, cost_usd real not null,
  input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null
);
create index if not exists prompt_index_model_id on prompt_index_model (id);

create table if not exists prompt_index_tool (id integer not null, tool text, kind text not null, calls integer not null);
create index if not exists prompt_index_tool_id on prompt_index_tool (id);

-- Turns that received events since they were indexed: recomputed on the next read.
create table if not exists index_dirty (prompt_id text primary key);

create virtual table if not exists prompt_fts using fts5(typed_text, tokenize = 'trigram');
"""

TABLES = ("index_meta", "index_dirty", "prompt_index", "prompt_index_model", "prompt_index_tool", "prompt_fts")


@dataclass
class Entry:
    """Everything the index stores about one prompt."""

    row: dict[str, Any]  # COLUMNS, and typed_text for prompt_fts
    models: list[dict[str, Any]] = field(default_factory=list)  # {model, *USAGE}
    tools: list[tuple[str | None, str, int]] = field(default_factory=list)  # (tool, kind, calls)


class PromptIndex:
    def __init__(self, db: sqlite3.Connection) -> None:
        self.db = db
        self.create()

    def create(self) -> None:
        try:
            for statement in filter(str.strip, SCHEMA.split(";")):  # not executescript: it commits an open transaction
                self.db.execute(statement)
        except sqlite3.OperationalError as e:  # the bundled sqlite of CPython builds has FTS5 (3.34+ for trigram)
            raise RuntimeError(f"GitPrompt needs SQLite with FTS5 and the trigram tokenizer (have {sqlite3.sqlite_version}): {e}") from e

    def drop(self) -> None:
        """Forget everything: the index is a cache."""
        for table in TABLES:
            self.db.execute(f"drop table if exists {table}")

    def invalidate(self) -> None:
        """Mark the whole index stale: the next `Indexer.ensure()` rebuilds it from the raw rows."""
        self.db.execute("delete from index_meta where key = 'rules_version'")

    @property
    def version(self) -> str | None:
        row = self.db.execute("select value from index_meta where key = 'rules_version'").fetchone()
        return row[0] if row else None

    @version.setter
    def version(self, value: str) -> None:
        self.db.execute("insert or replace into index_meta (key, value) values ('rules_version', ?)", (value,))

    def upsert(self, entries: Iterable[Entry]) -> int:
        """Replace the rows of these prompts (call inside a transaction for many)."""
        entries = list(entries)
        self.delete(e.row["id"] for e in entries)
        marks = ", ".join("?" * len(COLUMNS))
        self.db.executemany(
            f"insert into prompt_index ({', '.join(COLUMNS)}) values ({marks})", [[e.row[c] for c in COLUMNS] for e in entries]
        )
        self.db.executemany(
            f"insert into prompt_index_model (id, model, {', '.join(USAGE)}) values (?, ?, {', '.join('?' * len(USAGE))})",
            [[e.row["id"], m["model"], *(m[k] for k in USAGE)] for e in entries for m in e.models],
        )
        self.db.executemany(
            "insert into prompt_index_tool (id, tool, kind, calls) values (?, ?, ?, ?)",
            [(e.row["id"], *t) for e in entries for t in e.tools],
        )
        self.db.executemany(
            "insert into prompt_fts (rowid, typed_text) values (?, ?)",
            [(e.row["id"], e.row["typed_text"]) for e in entries if e.row["typed_text"]],
        )
        return len(entries)

    def delete(self, ids: Iterable[int]) -> None:
        keys = [(i,) for i in ids]
        for table, column in (("prompt_index", "id"), ("prompt_index_model", "id"), ("prompt_index_tool", "id"), ("prompt_fts", "rowid")):
            self.db.executemany(f"delete from {table} where {column} = ?", keys)

    def mark(self, prompt_ids: Iterable[str]) -> None:
        """Note that these turns changed (cheap: done on every ingest)."""
        self.db.executemany("insert or ignore into index_dirty (prompt_id) values (?)", [(p,) for p in prompt_ids])

    def take_marked(self) -> list[str]:
        """The marked turns, unmarked (call inside the transaction that reindexes them)."""
        return [r[0] for r in self.db.execute("delete from index_dirty returning prompt_id")]

    def unindexed(self) -> list[int]:
        """Prompts stored after the newest indexed one (an index lookup, not a scan)."""
        last = "select coalesce(max(id), 0) from prompt_index"
        return [r[0] for r in self.db.execute(f"select id from prompt where id > ({last})")]
