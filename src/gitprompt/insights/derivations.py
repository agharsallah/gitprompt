"""Metadata GitPrompt derives from what it stored. Computed on read, so a better rule applies to old
prompts too, and nothing here needs new access: it only reads the database (and git, in core/git.py).

Each value comes from a small function registered on one of three registries:

    PROMPT_STATS  the prompt text            -> PromptOut.stats
    TURN          the prompt + its events    -> PromptOut.summary
    SESSION       every event of the session -> PromptDetail.session

To add a derived value, write one function and decorate it with its registry and a description
of each value it returns. The descriptions are listed on the Fields page (DERIVED).

A prompt's origin and typed text come from the source spec's rules, not from code, so they are
derived in insights/origin.py (PromptOut.origin / typed_text / injected).
"""

import re
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from functools import cached_property
from typing import Any

from gitprompt.core.payload import compact
from gitprompt.insights import origin
from gitprompt.insights.redaction import find_secrets
from gitprompt.sources.spec import Spec


class Derivations[Context]:
    """An ordered set of functions, each deriving a few named values from the same context."""

    def __init__(self, prefix: str) -> None:
        self.prefix = prefix
        self._functions: list[Callable[[Context], dict[str, Any]]] = []
        self._docs: dict[str, str] = {}

    def __call__(self, docs: dict[str, str]) -> Callable:
        """Decorator: `docs` maps the value names (grouped with " / " when they share a meaning) to
        what they mean."""

        def register(fn: Callable[[Context], dict[str, Any]]) -> Callable[[Context], dict[str, Any]]:
            self._functions.append(fn)
            self._docs |= {f"{self.prefix}.{name}": meaning for name, meaning in docs.items()}
            return fn

        return register

    @property
    def docs(self) -> dict[str, str]:
        return dict(self._docs)

    def derive(self, context: Context) -> dict[str, Any]:
        out: dict[str, Any] = {}
        for fn in self._functions:
            out |= fn(context)
        return compact(out)


def _seconds(start: str, end: str) -> float:
    return round((datetime.fromisoformat(end) - datetime.fromisoformat(start)).total_seconds(), 1)


def _num(value: Any) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


# --- prompt text -----------------------------------------------------------------------------

PROMPT_STATS: Derivations[str] = Derivations("stats")


@PROMPT_STATS({"chars / words / lines / approx_tokens": "prompt size (tokens ≈ chars / 4)"})
def _size(text: str) -> dict[str, Any]:
    return {
        "chars": len(text),
        "words": len(text.split()),
        "lines": text.count("\n") + 1 if text else 0,
        "approx_tokens": -(-len(text) // 4),
    }


@PROMPT_STATS(
    {
        "slash_command": "command the prompt starts with, e.g. /review",
        "at_mentions / urls / code_blocks / questions": "what the prompt references",
    }
)
def _references(text: str) -> dict[str, Any]:
    return {
        "slash_command": (m := re.match(r"\s*(/[\w:-]+)", text)) and m[1],
        "at_mentions": re.findall(r"(?<!\S)@([\w./~-]+)", text),
        "urls": re.findall(r"https?://[^\s)>\]]+", text),
        "code_blocks": text.count("```") // 2,
        "questions": text.count("?"),
    }


@PROMPT_STATS({"secrets": "kinds of credentials found in the prompt (values never shown)"})
def _secrets(text: str) -> dict[str, Any]:
    return {"secrets": find_secrets(text)}


def prompt_stats(text: str) -> dict[str, Any]:
    return PROMPT_STATS.derive(text)


# --- one turn: the prompt and the events that share its prompt_id ---------------------------
# Event and tool names below are Claude Code's. Tools that copy its hook shape get these for free.

TURN_END = ("Stop", "StopFailure")
TOOL_RESULTS = ("PostToolUse", "PostToolUseFailure")
WRITE_TOOLS = {"Write", "Edit", "MultiEdit", "NotebookEdit"}
READ_TOOLS = {"Read", "Grep", "Glob", "LS"}
WEB_TOOLS = {"WebFetch", "WebSearch"}
USAGE_TOKENS = ("input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")

# What a tool call does (the dashboard draws the same kinds, web/src/lib/insights/tools.ts).
TOOL_KINDS = {
    **dict.fromkeys(("Read", "Grep", "Glob", "LS", "NotebookRead", "LSP"), "read"),
    **dict.fromkeys(("Write", "Edit", "MultiEdit", "NotebookEdit"), "write"),
    **dict.fromkeys(("Bash", "BashOutput", "KillShell", "PowerShell", "Monitor"), "shell"),
    **dict.fromkeys(("WebFetch", "WebSearch"), "web"),
    **dict.fromkeys(("Task", "Agent", "Workflow", "Skill", "SendMessage"), "agent"),
}


def tool_kind(name: str | None) -> str:
    """read, write, shell, web, mcp, agent or other."""
    if name and name.startswith("mcp__"):
        return "mcp"
    return TOOL_KINDS.get(name or "", "other")


@dataclass(frozen=True)
class Turn:
    prompt: dict[str, Any]  # a prompt_view row: received_at, turn, prev_at...
    events: list[dict[str, Any]]  # event rows, in arrival order

    def raws(self, *names: str) -> list[dict[str, Any]]:
        """Raw payloads of the events with one of these names."""
        return [e["raw"] for e in self.events if e["event"] in names]

    @cached_property
    def end(self) -> dict[str, Any] | None:
        """The last Stop/StopFailure event, if the turn has finished."""
        return next((e for e in reversed(self.events) if e["event"] in TURN_END), None)

    @cached_property
    def tool_uses(self) -> list[tuple[str | None, dict[str, Any]]]:
        """(tool name, tool input) of every finished tool call."""
        return [(t.get("tool_name"), t.get("tool_input") or {}) for t in self.raws(*TOOL_RESULTS)]

    def inputs(self, tools: set[str]) -> list[dict[str, Any]]:
        return [i for name, i in self.tool_uses if name in tools]


TURN: Derivations[Turn] = Derivations("summary")


@TURN(
    {
        "turn": "position of the prompt in its session",
        "since_previous_prompt_s": "think time since the previous prompt of the session",
    }
)
def _position(t: Turn) -> dict[str, Any]:
    p = t.prompt
    return {"turn": p.get("turn"), "since_previous_prompt_s": p.get("prev_at") and _seconds(p["prev_at"], p["received_at"])}


@TURN(
    {
        "response_s / ended_by / error": "time until Stop or StopFailure, and why it ended",
        "response": "Claude's final answer (Stop.last_assistant_message)",
    }
)
def _ending(t: Turn) -> dict[str, Any]:
    if not (end := t.end):
        return {}
    return {
        "response_s": _seconds(t.prompt["received_at"], end["received_at"]),
        "ended_by": end["event"],
        "error": end["raw"].get("error"),
        "response": end["raw"].get("last_assistant_message"),
    }


@TURN(
    {
        "tool_calls / tool_failures / tool_time_s": "tools used for the prompt (PostToolUse*)",
        "files_written / files_read": "file paths touched by file tools",
        "bash_commands / web": "shell commands run, URLs fetched and searches made",
        "mcp_tools": "MCP tools used",
        "tool_kinds": "tool calls by kind: read, write, shell, web, mcp, agent, other",
    }
)
def _tools(t: Turn) -> dict[str, Any]:
    return {
        "tool_calls": dict(Counter(name for name, _ in t.tool_uses)),
        "tool_failures": len(t.raws("PostToolUseFailure")),
        "tool_time_s": round(sum(_num(r.get("duration_ms")) for r in t.raws(*TOOL_RESULTS)) / 1000, 1),
        "files_written": sorted({i.get("file_path") or i.get("notebook_path") for i in t.inputs(WRITE_TOOLS)} - {None}),
        "files_read": sorted({i.get("file_path") or i.get("path") for i in t.inputs(READ_TOOLS)} - {None}),
        "bash_commands": [i.get("command") for i in t.inputs({"Bash"})],
        "web": [i.get("url") or i.get("query") for i in t.inputs(WEB_TOOLS)],
        "mcp_tools": sorted({name for name, _ in t.tool_uses if name and name.startswith("mcp__")}),
        "tool_kinds": dict(Counter(tool_kind(name) for name, _ in t.tool_uses)),
    }


@TURN(
    {
        "slash_command": "slash command the prompt expanded (UserPromptExpansion)",
        "subagents / tasks / notifications / permission_denied / compactions / model_switches": "agentic activity",
    }
)
def _agent_activity(t: Turn) -> dict[str, Any]:
    return {
        "slash_command": next((e.get("command_name") for e in t.raws("UserPromptExpansion")), None),
        "subagents": dict(Counter(e.get("agent_type") for e in t.raws("SubagentStart"))),
        "tasks": [e.get("task_subject") for e in t.raws("TaskCreated")],
        "notifications": dict(Counter(e.get("notification_type") for e in t.raws("Notification"))),
        "permission_denied": [e.get("tool_name") for e in t.raws("PermissionDenied")],
        "compactions": len(t.raws("PreCompact")),
        "model_switches": [e.get("to_model") for e in t.raws("PostModelSwitch")],
    }


@TURN(
    {
        "api_requests / models / *_tokens / cost_usd / api_time_s / api_errors": "usage from telemetry (needs the telemetry source)",
        "model_usage": "requests, tokens and cost per model (telemetry)",
        "tool_rejections": "tool calls you rejected (telemetry)",
    }
)
def _usage(t: Turn) -> dict[str, Any]:
    api = t.raws("api_request")
    by_model: dict[Any, list[dict[str, Any]]] = {}
    for e in api:
        by_model.setdefault(e.get("model"), []).append(e)
    return {
        **_usage_of(api),
        "models": dict(Counter(e.get("model") for e in api)),
        "model_usage": {model: _usage_of(requests) for model, requests in by_model.items()},
        "api_time_s": round(sum(_num(e.get("duration_ms")) for e in api) / 1000, 1),
        "api_errors": len(t.raws("api_error")),
        "tool_rejections": sum(e.get("decision") == "reject" for e in t.raws("tool_decision")),
    }


def _usage_of(api: list[dict[str, Any]]) -> dict[str, Any]:
    """Requests, tokens and cost of some api_request payloads."""
    return {
        "api_requests": len(api),
        **{k: int(sum(_num(e.get(k)) for e in api)) for k in USAGE_TOKENS},
        "cost_usd": round(sum(_num(e.get("cost_usd")) for e in api), 4),
    }


def turn(prompt: dict[str, Any], events: list[dict[str, Any]]) -> dict[str, Any]:
    """What happened while the tool worked on one prompt."""
    return TURN.derive(Turn(prompt, events))


# --- the whole session: every event that shares the prompt's session_id ----------------------

METRIC_PREFIX = "claude_code."  # telemetry metric names
ENVIRONMENT = ("app.", "terminal.", "os.", "host.", "service.", "vcs.", "user.", "organization.")  # OTel resource attributes

SESSION: Derivations[list[dict[str, Any]]] = Derivations("session")


def _raws(events: list[dict[str, Any]], name: str) -> list[dict[str, Any]]:
    return [e["raw"] for e in events if e["event"] == name]


@SESSION({"events / first_event_at / last_event_at": "events received for the session, by type, and when"})
def _activity(events: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "events": dict(Counter(e["event"] for e in events)),
        "first_event_at": events and events[0]["received_at"],
        "last_event_at": events and events[-1]["received_at"],
    }


@SESSION({"ended / instructions_loaded / cwd_changes / config_changes": "end reason, instruction files, directory and config changes"})
def _lifecycle(events: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "ended": next((r.get("reason") for r in reversed(_raws(events, "SessionEnd"))), None),
        "instructions_loaded": sorted({r.get("file_path") for r in _raws(events, "InstructionsLoaded")} - {None}),
        "cwd_changes": [r.get("new_cwd") for r in _raws(events, "CwdChanged")],
        "config_changes": [r.get("file_path") or r.get("source") for r in _raws(events, "ConfigChange")],
    }


@SESSION({"metric_totals": "telemetry metric totals: lines of code, commits, active time, tokens, cost"})
def _metrics(events: list[dict[str, Any]]) -> dict[str, Any]:
    totals: Counter[str] = Counter()
    for e in events:
        raw = e["raw"]
        if e["event"] and e["event"].startswith(METRIC_PREFIX) and "value" in raw:
            totals[e["event"] + (f"[{raw['type']}]" if raw.get("type") else "")] += _num(raw["value"])
    return {"metric_totals": {k: round(v, 4) for k, v in totals.items()}}


@SESSION({"environment": "app version, entrypoint, terminal, OS and account (telemetry resource attributes)"})
def _environment(events: list[dict[str, Any]]) -> dict[str, Any]:
    latest = next((e["raw"] for e in reversed(events) if "session.id" in e["raw"]), {})
    return {"environment": {k: v for k, v in latest.items() if k.startswith(ENVIRONMENT)}}


def session(events: list[dict[str, Any]]) -> dict[str, Any]:
    """Session-wide facts."""
    return SESSION.derive(events)


# --- everything derived about one prompt -----------------------------------------------------


def derive(row: dict[str, Any], events: list[dict[str, Any]], spec: Spec | None) -> dict[str, Any]:
    """A stored prompt row plus every value derived from it: origin and typed text (from the spec's
    rules), stats from its text, summary from the events of its turn. The prompt API returns this,
    and the prompt index (insights/indexer.py) stores a projection of it."""
    return row | origin.classify(row, events, spec) | {"stats": prompt_stats(row["prompt"] or ""), "summary": turn(row, events)}


# --- shown on the Fields page ----------------------------------------------------------------

DERIVED: dict[str, str] = {
    **origin.DOCS,
    **PROMPT_STATS.docs,
    **TURN.docs,
    **SESSION.docs,
    "commits / diffstat": "commits made after the prompt, until the next prompt of the session",
    "context.git_*": "repository state when the prompt was sent",
    "context.tool_version": "tool version when the prompt was sent",
}
