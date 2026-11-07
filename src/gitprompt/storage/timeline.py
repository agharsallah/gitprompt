"""Reads for stories and search (ADR-0016): prompts as the index sees them, in time order, with their
typed text and outcome, plus the few turn events a story shows. Every value comes from the prompt
index (storage/index.py) or the raw events; nothing here is stored.
"""

import sqlite3
from typing import Any

from gitprompt.storage.store import CHUNK, INDEXED, OUTCOME, where

# What a story or a search result needs of each prompt: index columns (with what insights/facts.py
# reads), the typed text, the outcome.
COLUMNS = (
    "i.id, i.prompt_id, i.source, i.session, i.repo, i.repo_name, i.branch, i.title, i.received_at, i.day, i.origin,"
    " i.chars, i.text_hash, i.slash_command, i.code_blocks, i.mentions, i.urls, i.cost_usd, i.api_requests,"
    " i.input_tokens, i.output_tokens, i.response_s, i.ended_by, i.tool_calls, i.tool_failures, i.subagents,"
    " i.files_written, i.git_commits, i.compactions, i.cache_read_tokens, i.cache_creation_tokens, i.reasoning_tokens,"
    " i.model, i.permission_mode, i.tool_version, i.answered"
)
TEXT = "(select typed_text from prompt_fts where rowid = i.id) typed_text"


def prompts(db: sqlite3.Connection, limit: int | None = None, newest_first: bool = False, **filters: Any) -> list[dict[str, Any]]:
    """The prompts the filters keep (store.FILTERS), oldest first unless `newest_first`."""
    sql, params = where(**filters)
    order = "desc" if newest_first else "asc"
    query = f"select {COLUMNS}, {OUTCOME} outcome, {TEXT} from {INDEXED} where {sql} order by i.received_at {order}, i.id {order}"
    if limit is not None:
        query, params = query + " limit ?", [*params, limit]
    return [dict(r) for r in db.execute(query, params)]


def one(db: sqlite3.Connection, id: int) -> dict[str, Any] | None:
    """One prompt, as `prompts` returns it."""
    row = db.execute(f"select {COLUMNS}, {OUTCOME} outcome, {TEXT} from {INDEXED} where i.id = ?", (id,)).fetchone()
    return dict(row) if row else None


def count(db: sqlite3.Connection, **filters: Any) -> int:
    sql, params = where(**filters)
    return db.execute(f"select count(*) from {INDEXED} where {sql}", params).fetchone()[0]


def tool_kinds(db: sqlite3.Connection, ids: list[int]) -> dict[int, dict[str, int]]:
    """Tool calls per kind (read, write, shell...) of each prompt's turn."""
    out: dict[int, dict[str, int]] = {}
    for chunk in _chunks(ids):
        rows = db.execute(
            f"select id, kind, sum(calls) n from prompt_index_tool where id in ({','.join('?' * len(chunk))}) group by 1, 2", chunk
        )
        for r in rows:
            out.setdefault(r["id"], {})[r["kind"]] = r["n"]
    return out


# One row per tool call or compaction of a turn, with only the fields a story shows: the tool's input
# (JSON text: the files a write tool touched, insights.derivations.files_of), and what a subagent was
# asked (derivations TASK_INPUTS / BRIEF_INPUTS: Claude Code's Agent tool, Codex's spawn_agent).
_TURN_EVENTS = """select prompt_id, event, received_at, json_extract(raw, '$.tool_name') tool,
  case json_type(raw, '$.tool_input') when 'object' then json_extract(raw, '$.tool_input') end tool_input,
  coalesce(json_extract(raw, '$.tool_input.description'), json_extract(raw, '$.tool_input.task_name')) description,
  json_extract(raw, '$.tool_input.subagent_type') subagent_type,
  substr(coalesce(json_extract(raw, '$.tool_input.prompt'), json_extract(raw, '$.tool_input.message')), 1, ?) prompt
from event where event in ('PostToolUse', 'PreCompact') and prompt_id in ({marks}) order by id"""


def turn_events(db: sqlite3.Connection, prompt_ids: list[str], clip: int = 600) -> list[dict[str, Any]]:
    """Finished tool calls and compactions of these turns, oldest first."""
    out: list[dict[str, Any]] = []
    for chunk in _chunks(prompt_ids):
        out += [dict(r) for r in db.execute(_TURN_EVENTS.format(marks=",".join("?" * len(chunk))), [clip, *chunk])]
    return out


def _chunks(values: list) -> list[list]:
    return [values[i : i + CHUNK] for i in range(0, len(values), CHUNK)]
