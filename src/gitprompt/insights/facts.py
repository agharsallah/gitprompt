"""A prompt's facts: the few values every page shows about a turn, computed in one place (ADR-0021).

The prompt list and detail, session and project stories, search results and exports all read
`facts`, never pick their own fields out of `fields` and `summary`. Facts are built from an index
row (storage/index.py COLUMNS, as insights/indexer.py `entry` projects them from the derivations)
and the source spec, so they need no `if source == ...`:

    connector       the tool that sent the prompt: spec id, name, badge label, who answers
    model           the turn's main model: from model calls when the tool reports them, else the
                    model its hooks carried (`model_source` says which)
    tokens          input, output, cache and reasoning tokens; null when no model call reported any
                    (the tool can't, not in this version, or the call was cancelled), never zeros
    cost_usd        null unless a model call reported a cost (the spec's `usage` maps one)
    outcome         done, failed, interrupted, superseded, running, open (storage/store.py OUTCOME)
    ...             permission mode, tool version, time, tool calls, files, commits, subagents

What a tool can report at all is spec data too (`capabilities`, `/api/sources`): the dashboard says
"not available" or "needs version X" from that, and shows facts as they are.
"""

from collections.abc import Mapping
from typing import Any

from gitprompt.sources.spec import Spec

# The facts of a prompt, each with what it means (listed on the Fields page).
DOCS: dict[str, str] = {
    "facts.connector": "the tool that sent the prompt: spec id, name, short name for badges, and who answers",
    "facts.model / model_source": "the turn's main model, from its model calls (usage) or the model its hooks reported (hook)",
    "facts.models": "every model the turn used",
    "facts.tokens": "input, output, cache read, cache write, reasoning and total tokens; null when no model call reported any",
    "facts.cost_usd": "the turn's cost; null unless a model call reported one",
    "facts.api_requests": "model calls reported for the turn",
    "facts.outcome": "how the turn ended: done, failed, interrupted, superseded, running or open",
    "facts.permission_mode / tool_version": "as the tool reported them with the prompt",
    "facts.response_s / answered": "time until the turn ended, and whether the end carried the final answer",
    "facts.tool_calls / tool_failures / files_written / git_commits / subagents": "what the turn did",
}

TOKENS = {  # facts.tokens key -> index column
    "input": "input_tokens",
    "output": "output_tokens",
    "cache_read": "cache_read_tokens",
    "cache_creation": "cache_creation_tokens",
    "reasoning": "reasoning_tokens",
}


def main_model(summary: Mapping[str, Any], fields: Mapping[str, Any]) -> str | None:
    """The model of most model calls of a turn, else the one the tool reported with the prompt."""
    calls = {m: n for m, n in (summary.get("models") or {}).items() if m}
    if calls:
        return max(calls, key=lambda m: (calls[m], m))
    return fields.get("model") or None


def connector(source: str, spec: Spec | None) -> dict[str, str]:
    if not spec:
        return {"id": source, "name": source, "label": source, "assistant": ""}
    return {"id": spec.id, "name": spec.name, "label": spec.label, "assistant": spec.assistant}


def _tokens(row: Mapping[str, Any]) -> dict[str, int] | None:
    if not row.get("api_requests"):
        return None
    out = {name: int(row.get(column) or 0) for name, column in TOKENS.items()}
    # Reasoning tokens are part of the output tokens: the total counts them once.
    total = out["input"] + out["output"] + out["cache_read"] + out["cache_creation"]
    # Calls that carried no counts (Codex reports none for a cancelled call) are not zero tokens.
    return out | {"total": total} if total else None


def _cost(row: Mapping[str, Any], spec: Spec | None) -> float | None:
    cost = row.get("cost_usd") or 0
    reports = spec is not None and "cost_usd" in spec.usage.values
    return round(cost, 4) if row.get("api_requests") and (cost or reports) else None


def facts(row: Mapping[str, Any], spec: Spec | None, outcome: str | None = None, models: list[str] | None = None) -> dict[str, Any]:
    """The facts of one prompt from its index row (storage/index.py COLUMNS)."""
    model = row.get("model")
    return {
        "connector": connector(row["source"], spec),
        "model": model,
        "models": models if models is not None else ([model] if model else []),
        "model_source": ("usage" if row.get("api_requests") else "hook") if model else None,
        "tokens": _tokens(row),
        "cost_usd": _cost(row, spec),
        "api_requests": int(row.get("api_requests") or 0),
        "outcome": outcome,
        "permission_mode": row.get("permission_mode"),
        "tool_version": row.get("tool_version"),
        "response_s": row.get("response_s"),
        "answered": bool(row.get("answered")),
        "tool_calls": int(row.get("tool_calls") or 0),
        "tool_failures": int(row.get("tool_failures") or 0),
        "files_written": int(row.get("files_written") or 0),
        "git_commits": int(row.get("git_commits") or 0),
        "subagents": int(row.get("subagents") or 0),
    }


def attach(rows: list[dict[str, Any]], specs: Mapping[str, Spec]) -> list[dict[str, Any]]:
    """Index rows (storage/timeline.py, with their `outcome`) with their facts added, in place."""
    for row in rows:
        row["facts"] = facts(row, specs.get(row["source"]), row.get("outcome"))
    return rows
