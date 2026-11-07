"""Statistics over the prompt index (storage/index.py): counts, sums and response-time quantiles,
for every prompt the filters keep (store.FILTERS), optionally grouped, and each group optionally
split by a second grouping (ADR-0012, ADR-0014).

To add a grouping, add a line to GROUPS and its name to GroupBy. A prompt has one value for most
groupings; for `model`, `tool`, `tool_kind`, `work` and `query_source` it counts in every group it
touched, and the usage (or tool call) measures of those groups are split exactly between them.

`telemetry()` is the one statistic that is not on the index: telemetry metrics (lines of code,
commits...) and the slash commands the hooks never see arrive per session, not per prompt, so they
are summed over the sessions of the prompts the filters keep.
"""

import json
import sqlite3
from dataclasses import dataclass, field
from typing import Any, Literal

from gitprompt.storage.store import FOLLOWED, INDEXED, OUTCOME, SIZE, buckets, where

GroupBy = Literal[
    "day", "day_hour", "hour", "weekday", "weekday_hour", "origin", "source", "surface", "session", "session_length", "repo", "branch",
    "command", "size", "text", "outcome", "model", "tool", "tool_kind", "work", "query_source",
]  # fmt: skip
Order = Literal["key", "prompts", "sessions", "cost_usd", "last_at"]


@dataclass(frozen=True)
class Group:
    key: str  # SQL for the group's key (text, or null)
    join: str = ""  # one row per value, when a prompt can have several
    measures: dict[str, str] = field(default_factory=dict)  # measures that replace (or add to) the prompt-level ones
    by_key: bool = False  # ordered by key (time buckets, size buckets) rather than by prompt count
    detail: str = "null"  # SQL for one more fact about the group: a name, a kind
    excerpt: Literal["first", "last", None] = None  # else, the start of the group's first or last typed text


_USAGE = ("api_requests", "cost_usd", "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")
_MODEL = "join prompt_index_model m on m.id = i.id"
_TOOL = "join prompt_index_tool t on t.id = i.id"
_WORK = "join prompt_index_work w on w.id = i.id"
# One row per prompt and kind of work (several query sources map to one kind).
_WORK_KIND = (
    "join (select id, work, " + ", ".join(f"total({k}) {k}" for k in _USAGE) + " from prompt_index_work group by id, work) w on w.id = i.id"
)


def _split(alias: str) -> dict[str, str]:
    return {k: f"total({alias}.{k})" for k in _USAGE}


_EPOCH_S = "(julianday(i.received_at) * 86400.0)"
# Seconds from a session's first prompt to the end of its last turn, over every prompt indexed for it.
_SPAN = f"(max({_EPOCH_S} + coalesce(i.response_s, 0)) - min({_EPOCH_S}))"
SESSION_LENGTHS = (0, 60, 300, 900, 1800, 3600, 7200, 14400)  # seconds
_SESSIONS = (
    f"join (select i.session sid, {_SPAN} span from prompt_index i where i.session is not null group by i.session) ss on ss.sid = i.session"
)

GROUPS: dict[str, Group] = {
    "day": Group("i.day", by_key=True),  # local calendar day, YYYY-MM-DD
    "day_hour": Group("i.day || ' ' || printf('%02d', i.hour)", by_key=True),  # local clock hour, YYYY-MM-DD HH
    "hour": Group("printf('%02d', i.hour)", by_key=True),  # local hour, 00-23
    "weekday": Group("cast(i.weekday as text)", by_key=True),  # ISO weekday, 1 = Monday
    "weekday_hour": Group("i.weekday || '-' || printf('%02d', i.hour)", by_key=True),  # e.g. 1-09
    "origin": Group("i.origin"),
    "source": Group("i.source"),  # detail: the tool's name (api/routes/stats.py, from its spec)
    "surface": Group("i.surface"),  # where it was typed; detail: its name (api/routes/stats.py)
    # detail: the session's title, else the start of its first prompt; span_s: first prompt to last turn's end
    "session": Group("i.session", measures={"span_s": f"round({_SPAN}, 1)"}, detail="max(i.title)", excerpt="first"),
    # sessions by how long they lasted (all their prompts, whatever the filters): key = lower bound in seconds
    "session_length": Group(buckets("ss.span", SESSION_LENGTHS), _SESSIONS, by_key=True),
    "repo": Group("i.repo", detail="max(i.repo_name)"),  # detail: owner/name of the remote
    "branch": Group("i.branch"),  # git branch when the prompt was sent
    "command": Group("i.slash_command"),
    "size": Group(SIZE, by_key=True),  # typed characters: key = lower bound (store.SIZES)
    "text": Group("i.text_hash", excerpt="last"),  # the same typed text; detail: its start
    "outcome": Group(OUTCOME),  # done, failed, superseded, running, open
    "model": Group("m.model", _MODEL, _split("m")),
    "tool": Group("t.tool", _TOOL, {"tool_calls": "total(t.calls)"}, detail="max(t.kind)"),  # detail: the tool's kind
    "tool_kind": Group("t.kind", _TOOL, {"tool_calls": "total(t.calls)"}),
    "work": Group("w.work", _WORK_KIND, _split("w")),  # main, subagent, system (derivations.work_kind)
    "query_source": Group("w.query_source", _WORK, _split("w"), detail="max(w.work)"),  # detail: its kind of work
}

# total() is sum() that gives 0.0 instead of null for no rows; total(condition) counts the rows.
MEASURES = {
    "prompts": "count(*)",
    "sessions": "count(distinct i.session)",
    "api_requests": "total(i.api_requests)",
    "cost_usd": "total(i.cost_usd)",
    "input_tokens": "total(i.input_tokens)",
    "output_tokens": "total(i.output_tokens)",
    "cache_read_tokens": "total(i.cache_read_tokens)",
    "cache_creation_tokens": "total(i.cache_creation_tokens)",
    "api_time_s": "round(total(i.api_time_s), 1)",
    "api_errors": "total(i.api_errors)",
    "tool_calls": "total(i.tool_calls)",
    "tool_failures": "total(i.tool_failures)",
    "tool_time_s": "round(total(i.tool_time_s), 1)",
    "subagent_tool_calls": "total(i.subagent_tool_calls)",
    "subagents": "total(i.subagents)",
    "files_written": "total(i.files_written)",
    "git_commits": "total(i.git_commits)",
    "compactions": "total(i.compactions)",
    "denials": "total(i.denials)",
    "rejections": "total(i.rejections)",
    "ended": "count(i.response_s)",  # prompts whose turn has an end (and so a response time)
    "failed": "total(i.ended_by = 'StopFailure')",
    "followed": f"total({FOLLOWED})",  # prompts with a later prompt in their session
    "with_code": "total(i.code_blocks > 0)",
    "with_url": "total(i.urls > 0)",
    "with_mention": "total(i.mentions > 0)",
    "with_secret": "total(i.secrets > 0)",
    "with_commit": "total(i.git_commits > 0)",
    "distinct_texts": "count(distinct i.text_hash)",  # prompts minus this = prompts that repeat a text
    "first_at": "min(i.received_at)",
    "last_at": "max(i.received_at)",
}
QUANTILES = {"response_s_p50": 0.5, "response_s_p90": 0.9}

# The response times of a bucket, in one string: parsing it is faster than fetching a row per prompt.
TIMES = "group_concat(i.response_s) times"


def quantile(sorted_values: list[float], q: float) -> float | None:
    """Linear-interpolated quantile."""
    if not sorted_values:
        return None
    pos = (len(sorted_values) - 1) * q
    lo, hi = int(pos), min(int(pos) + 1, len(sorted_values) - 1)
    return round(sorted_values[lo] + (sorted_values[hi] - sorted_values[lo]) * (pos - lo), 1)


def _select(*groups: Group) -> str:
    measures: dict[str, str] = dict(MEASURES)
    for g in groups:
        measures |= g.measures
    return ", ".join(f"{sql} {name}" for name, sql in measures.items()) + f", {TIMES}"


def _order(group: Group, order: str | None, key: str = "key") -> str:
    order = order or ("key" if group.by_key else "prompts")
    return f"{key}" if order == "key" else f"{order} desc, {key}"


def aggregate(
    db: sqlite3.Connection,
    group_by: str | None = None,
    limit: int = 1000,
    split_by: str | None = None,
    order: str | None = None,
    **filters: Any,
) -> dict[str, Any]:
    """{totals, groups} for the prompts the filters keep. `groups` is empty without `group_by`; with
    `split_by`, each group holds its `parts`: the same measures for each value of that grouping."""
    sql, params = where(**filters)
    totals = _bucket(db.execute(f"select null key, null detail, {_select()}, {_IDS} from {INDEXED} where {sql}", params).fetchone())
    if not group_by:
        return {"totals": totals, "groups": []}
    group = GROUPS[group_by]
    select = f"select {group.key} key, {group.detail} detail, {_select(group)}, {_IDS} from {INDEXED} {group.join}"
    inner = f"{select} where {sql} group by 1 order by {_order(group, order)} limit ?"
    rows = db.execute(_with_excerpt(inner, group), [*params, limit])
    groups = [_bucket(r) for r in rows]
    if split_by:
        _add_parts(db, groups, group, split_by, sql, params)
    return {"totals": totals, "groups": groups}


# The first and last prompt of a bucket, to show an excerpt of its text (an aggregate can't be used in
# a subquery of the same select, so the excerpt is read in an outer one).
_IDS = "min(i.id) first_id, max(i.id) last_id"
_TEXT = "(select substr(typed_text, 1, 200) from prompt_fts where rowid = q.{column})"


def _with_excerpt(inner: str, group: Group) -> str:
    if not group.excerpt:
        return inner
    return f"select q.*, coalesce(q.detail, {_TEXT.format(column=group.excerpt + '_id')}) excerpt from ({inner}) q"


def _add_parts(db: sqlite3.Connection, groups: list[dict], group: Group, split_by: str, sql: str, params: list) -> None:
    split = GROUPS[split_by]
    if group is split:
        raise ValueError("split_by must differ from group_by")
    if group.join and split.join:
        raise ValueError("group_by and split_by can't both be groupings with several values per prompt")
    keys = [g["key"] for g in groups if g["key"] is not None]
    nulls = any(g["key"] is None for g in groups)
    kept = f"({group.key} in (select value from json_each(?)) or ({group.key} is null and ?))"
    inner = (
        f"select {group.key} parent, {split.key} key, {split.detail} detail, {_select(group, split)}, {_IDS}"
        f" from {INDEXED} {group.join or split.join} where {sql} and {kept}"
        f" group by 1, 2 order by parent, {_order(split, None)}"
    )
    rows = db.execute(_with_excerpt(inner, split), [*params, json.dumps(keys), nulls])
    parts: dict[Any, list[dict]] = {}
    for r in rows:
        bucket = dict(r)
        parts.setdefault(bucket.pop("parent"), []).append(_bucket(bucket))
    for g in groups:
        g["parts"] = parts.get(g["key"], [])


def _bucket(row: sqlite3.Row | dict) -> dict[str, Any]:
    out = dict(row)
    del out["first_id"], out["last_id"]
    if "excerpt" in out:
        out["detail"] = out.pop("excerpt")
    times = sorted(map(float, out.pop("times").split(","))) if out["times"] else []
    return out | {name: quantile(times, q) for name, q in QUANTILES.items()}


# --- per session: telemetry metrics and slash commands -----------------------------------------


def telemetry(db: sqlite3.Connection, **filters: Any) -> dict[str, Any]:
    """Telemetry metric totals (named like derivations.SESSION `metric_totals`) and slash commands
    (storage/index.py session_*), over the sessions of the prompts the filters keep, and within
    `since`/`until` when given."""
    sql, params = where(**filters)
    sessions = f"select distinct i.session from {INDEXED} where {sql} and i.session is not null"
    window, window_params = "", []
    for key, op in (("since", ">="), ("until", "<")):
        if filters.get(key):
            window += f" and t.received_at {op} ?"
            window_params.append(filters[key])
    scope, scope_params = f"t.session in ({sessions}){window}", [*params, *window_params]
    metrics = db.execute(f"select name, total(value) value from session_metric t where {scope} group by 1 order by 1", scope_params)
    commands = db.execute(
        "select command, source, count(*) uses, count(distinct session) sessions"
        f" from session_command t where {scope} group by 1, 2 order by uses desc, command",
        scope_params,
    )
    return {
        "sessions": db.execute(f"select count(*) from ({sessions})", params).fetchone()[0],
        "metrics": {r["name"]: round(r["value"], 4) for r in metrics},
        "commands": [dict(r) for r in commands],
    }
