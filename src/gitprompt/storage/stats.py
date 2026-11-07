"""Statistics over the prompt index (storage/index.py): counts, sums and response-time quantiles,
for every prompt the filters keep (store.FILTERS), optionally grouped.

To add a grouping, add a line to GROUPS and its name to GroupBy. A prompt has one value for most
groupings; for `model`, `tool` and `tool_kind` it counts in every group it touched, and the usage
(or tool call) measures of those groups are split exactly between them.
"""

import sqlite3
from dataclasses import dataclass, field
from typing import Any, Literal

from gitprompt.storage.store import INDEXED, where

GroupBy = Literal[
    "day", "hour", "weekday", "weekday_hour", "origin", "source", "session", "repo", "command", "outcome", "model", "tool", "tool_kind"
]


@dataclass(frozen=True)
class Group:
    key: str  # SQL for the group's key (text, or null)
    join: str = ""  # one row per value, when a prompt can have several
    measures: dict[str, str] = field(default_factory=dict)  # measures that replace the prompt-level ones
    by_key: bool = False  # ordered by key (time buckets) rather than by prompt count


_MODEL = "join prompt_index_model m on m.id = i.id"
_TOOL = "join prompt_index_tool t on t.id = i.id"
_SPLIT_USAGE = {
    k: f"total(m.{k})" for k in ("api_requests", "cost_usd", "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")
}

GROUPS: dict[str, Group] = {
    "day": Group("i.day", by_key=True),  # local calendar day, YYYY-MM-DD
    "hour": Group("printf('%02d', i.hour)", by_key=True),  # local hour, 00-23
    "weekday": Group("cast(i.weekday as text)", by_key=True),  # ISO weekday, 1 = Monday
    "weekday_hour": Group("i.weekday || '-' || printf('%02d', i.hour)", by_key=True),  # e.g. 1-09
    "origin": Group("i.origin"),
    "source": Group("i.source"),
    "session": Group("i.session"),
    "repo": Group("i.repo"),
    "command": Group("i.slash_command"),
    "outcome": Group("i.ended_by"),  # Stop, StopFailure, or null: no end recorded
    "model": Group("m.model", _MODEL, _SPLIT_USAGE),
    "tool": Group("t.tool", _TOOL, {"tool_calls": "total(t.calls)"}),
    "tool_kind": Group("t.kind", _TOOL, {"tool_calls": "total(t.calls)"}),
}

# total() is sum() that gives 0.0 instead of null for no rows.
MEASURES = {
    "prompts": "count(*)",
    "sessions": "count(distinct i.session)",
    "api_requests": "total(i.api_requests)",
    "cost_usd": "total(i.cost_usd)",
    "input_tokens": "total(i.input_tokens)",
    "output_tokens": "total(i.output_tokens)",
    "cache_read_tokens": "total(i.cache_read_tokens)",
    "cache_creation_tokens": "total(i.cache_creation_tokens)",
    "tool_calls": "total(i.tool_calls)",
    "tool_failures": "total(i.tool_failures)",
    "ended": "count(i.response_s)",  # prompts whose turn has an end (and so a response time)
    "first_at": "min(i.received_at)",
    "last_at": "max(i.received_at)",
}
QUANTILES = {"response_s_p50": 0.5, "response_s_p90": 0.9}


def quantile(sorted_values: list[float], q: float) -> float | None:
    """Linear-interpolated quantile, as the dashboard computes it (insights/aggregate.ts)."""
    if not sorted_values:
        return None
    pos = (len(sorted_values) - 1) * q
    lo, hi = int(pos), min(int(pos) + 1, len(sorted_values) - 1)
    return round(sorted_values[lo] + (sorted_values[hi] - sorted_values[lo]) * (pos - lo), 1)


def _measures(group: Group | None) -> str:
    return ", ".join(f"{sql} {name}" for name, sql in (MEASURES | (group.measures if group else {})).items())


def aggregate(db: sqlite3.Connection, group_by: str | None = None, limit: int = 1000, **filters: Any) -> dict[str, Any]:
    """{totals, groups} for the prompts the filters keep. `groups` is empty without `group_by`."""
    sql, params = where(**filters)
    measures = f"{_measures(None)}, {TIMES}"
    totals = _bucket(db.execute(f"select null key, {measures} from {INDEXED} where {sql}", params).fetchone())
    if not group_by:
        return {"totals": totals, "groups": []}
    group = GROUPS[group_by]
    order = "key" if group.by_key else "prompts desc, key"
    select = f"select {group.key} key, {_measures(group)}, {TIMES} from {INDEXED} {group.join}"
    rows = db.execute(f"{select} where {sql} group by key order by {order} limit ?", [*params, limit])
    return {"totals": totals, "groups": [_bucket(r) for r in rows]}


# The response times of a bucket, in one string: parsing it is faster than fetching a row per prompt.
TIMES = "group_concat(i.response_s) times"


def _bucket(row: sqlite3.Row) -> dict[str, Any]:
    out = dict(row)
    times = sorted(map(float, out.pop("times").split(","))) if out["times"] else []
    return out | {name: quantile(times, q) for name, q in QUANTILES.items()}
