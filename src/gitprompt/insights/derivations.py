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
from dataclasses import dataclass, field
from datetime import datetime
from functools import cached_property
from typing import Any

from gitprompt.core.payload import compact, pluck
from gitprompt.insights import origin
from gitprompt.insights.redaction import find_secrets
from gitprompt.sources.spec import Spec, ToolKind, Usage, by_name, tool_kind


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
# Event names below are Claude Code's; tools that copy its hook shape get these for free. What each
# tool does comes from the source spec (`tool_kinds`), and its inputs are read by generic names.

TURN_END = ("Stop", "StopFailure", "Interrupt")  # Interrupt: you stopped the turn (Codex)
TOOL_RESULTS = ("PostToolUse", "PostToolUseFailure")
USAGE_TOKENS = ("input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")
FILE_INPUTS = ("file_path", "notebook_path", "path")  # tool inputs naming the file a tool touched
# What an agent tool (spec tool_kinds: agent) was asked: its task's name and its brief, by the input
# names the tools use (Claude Code's Agent tool: description, prompt; Codex's spawn_agent: task_name,
# message).
TASK_INPUTS = ("description", "task_name")
BRIEF_INPUTS = ("prompt", "message")
ERROR_EVENTS = ("ErrorOccurred",)  # an error the tool reported during the turn (Copilot CLI)
TEXT_CHARS = 280  # of an error or a brief kept in a summary

GIT_COMMIT = re.compile(r"\bgit\b(?:\s+-\S+(?:\s+\S+)?)*\s+commit\b")  # git commit, git -C dir commit


def _first(data: dict[str, Any], keys: tuple[str, ...]) -> Any:
    return next((data[k] for k in keys if data.get(k)), None)


def _file(tool_input: dict[str, Any]) -> Any:
    return _first(tool_input, FILE_INPUTS)


def _text(value: Any) -> str | None:
    """A message, clipped for a summary."""
    if value in (None, ""):
        return None
    text = value if isinstance(value, str) else str(value)
    return text if len(text) <= TEXT_CHARS else text[:TEXT_CHARS].rstrip() + "…"


def files_of(name: str | None, tool_input: Any, tool_files: dict[str, re.Pattern[str]]) -> list[str]:
    """Files one tool call touched: its path input, else what the spec's `tool_files` pattern for the
    tool finds in its text inputs (the files a patch adds, updates or deletes)."""
    if not isinstance(tool_input, dict):
        return []
    if path := _file(tool_input):
        return [path]
    if not (pattern := by_name(tool_files, name)):
        return []
    return [m[1].strip() for v in tool_input.values() if isinstance(v, str) for m in pattern.finditer(v)]


@dataclass(frozen=True)
class Turn:
    prompt: dict[str, Any]  # a prompt_view row: received_at, turn, prev_at...
    events: list[dict[str, Any]]  # event rows, in arrival order
    tool_kinds: dict[str, ToolKind] = field(default_factory=dict)  # the source spec's `tool_kinds`
    tool_files: dict[str, re.Pattern[str]] = field(default_factory=dict)  # ...and its `tool_files`
    usage: Usage = field(default_factory=Usage)  # ...and its `usage`

    def raws(self, *names: str) -> list[dict[str, Any]]:
        """Raw payloads of the events with one of these names."""
        return [e["raw"] for e in self.events if e["event"] in names]

    @cached_property
    def end(self) -> dict[str, Any] | None:
        """The last Stop/StopFailure/Interrupt event, if the turn has finished."""
        return next((e for e in reversed(self.events) if e["event"] in TURN_END), None)

    @cached_property
    def tool_uses(self) -> list[tuple[str | None, dict[str, Any]]]:
        """(tool name, tool input) of every finished tool call."""
        return [(t.get("tool_name"), t.get("tool_input") or {}) for t in self.raws(*TOOL_RESULTS)]

    def kind(self, name: str | None) -> ToolKind:
        return tool_kind(self.tool_kinds, name)

    def inputs(self, kind: ToolKind) -> list[dict[str, Any]]:
        """Inputs of the finished calls of tools of this kind."""
        return [i for name, i in self.tool_uses if self.kind(name) == kind]

    def files(self, kind: ToolKind) -> list[str]:
        """Files touched by the finished calls of tools of this kind."""
        return sorted({f for name, i in self.tool_uses if self.kind(name) == kind for f in files_of(name, i, self.tool_files)})


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
        "response_s / ended_by / error": "time until Stop, StopFailure or Interrupt, and how it ended",
        "response": "the agent's final answer (Stop.last_assistant_message)",
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
        "files_written / files_read": "file paths touched by write and read tools (a path input, or the spec's tool_files pattern)",
        "bash_commands / web": "shell commands run, URLs fetched and searches made",
        "mcp_tools": "MCP tools used",
        "tool_kinds / tool_kind_of": "calls per kind (spec tool_kinds: read, write, shell, web, mcp, agent, other), and each tool's kind",
        "subagent_tool_calls": "tool calls made by subagents (their hook payload has agent_id)",
        "git_commits": "git commits the agent ran (shell commands with `git commit`)",
        "tool_errors": "what the failed tool calls reported (PostToolUseFailure.error)",
        "subagent_tasks": "what each subagent was asked: task name and brief (agent tools' inputs)",
    }
)
def _tools(t: Turn) -> dict[str, Any]:
    commands = [i["command"] for i in t.inputs("shell") if i.get("command")]
    return {
        "tool_calls": dict(Counter(name for name, _ in t.tool_uses)),
        "tool_failures": len(t.raws("PostToolUseFailure")),
        "tool_time_s": round(sum(_num(r.get("duration_ms")) for r in t.raws(*TOOL_RESULTS)) / 1000, 1),
        "files_written": t.files("write"),
        "files_read": t.files("read"),
        "bash_commands": commands,
        "web": [i.get("url") or i.get("query") for i in t.inputs("web")],
        "mcp_tools": sorted({name for name, _ in t.tool_uses if name and t.kind(name) == "mcp"}),
        "tool_kinds": dict(Counter(t.kind(name) for name, _ in t.tool_uses)),
        "tool_kind_of": {name: t.kind(name) for name, _ in t.tool_uses if name},
        "subagent_tool_calls": sum(bool(r.get("agent_id")) for r in t.raws(*TOOL_RESULTS)),
        "git_commits": sum(bool(GIT_COMMIT.search(str(c))) for c in commands),
        "tool_errors": [e for r in t.raws("PostToolUseFailure") if (e := _text(r.get("error")))],
        "subagent_tasks": [
            {"task": _first(i, TASK_INPUTS), "brief": _text(_first(i, BRIEF_INPUTS))}
            for i in t.inputs("agent")
            if _first(i, TASK_INPUTS + BRIEF_INPUTS)
        ],
    }


@TURN(
    {
        "slash_command": "slash command the prompt expanded (UserPromptExpansion, or telemetry user_prompt.command_name)",
        "subagents / tasks / notifications / permission_denied / compactions / model_switches": "agentic activity",
        "compaction_triggers": "what started each compaction: auto (the context was full) or manual",
        "errors": "errors the tool reported during the turn (ErrorOccurred): name, message, where, recoverable",
    }
)
def _agent_activity(t: Turn) -> dict[str, Any]:
    return {
        "slash_command": next((e.get("command_name") for e in t.raws("UserPromptExpansion", "user_prompt") if e.get("command_name")), None),
        "subagents": dict(Counter(e.get("agent_type") for e in t.raws("SubagentStart"))),
        "tasks": [e.get("task_subject") for e in t.raws("TaskCreated")],
        "notifications": dict(Counter(e.get("notification_type") for e in t.raws("Notification"))),
        "permission_denied": [e.get("tool_name") for e in t.raws("PermissionDenied")],
        "compactions": len(t.raws("PreCompact")),
        "model_switches": [e.get("to_model") for e in t.raws("PostModelSwitch")],
        "compaction_triggers": dict(Counter(e.get("trigger") for e in t.raws("PreCompact") if e.get("trigger"))),
        "errors": [
            compact(
                {
                    "name": pluck(e, "error.name"),
                    "message": _text(pluck(e, "error.message") or e.get("error")),
                    "context": e.get("error_context"),
                    "recoverable": e.get("recoverable"),
                }
            )
            for e in t.raws(*ERROR_EVENTS)
        ],
    }


@TURN(
    {
        "api_requests / models / *_tokens / cost_usd / api_time_s / api_errors": "model calls (the spec's `usage` events)",
        "reasoning_tokens": "output tokens spent reasoning, when the tool reports them",
        "model_call_status": "model calls per status the tool reported: completed, failed, cancelled",
        "model_usage": "requests, tokens and cost per model",
        "work_usage": "requests, tokens and cost per query_source: the main thread, a subagent, or a system call (telemetry)",
        "tool_rejections": "tool calls you rejected (telemetry)",
    }
)
def _usage(t: Turn) -> dict[str, Any]:
    calls = model_calls(t.raws(*t.usage.events), t.usage)
    return {
        **_usage_of(calls),
        "reasoning_tokens": int(sum(_num(c.get("reasoning_tokens")) for c in calls)),
        "model_call_status": dict(Counter(c["status"] for c in calls if c.get("status"))),
        "models": dict(Counter(c.get("model") for c in calls)),
        "model_usage": {model: _usage_of(group) for model, group in _by(calls, "model").items()},
        "work_usage": {source: _usage_of(group) for source, group in _by(calls, "query_source").items()},
        "api_time_s": round(sum(_num(c.get("duration_ms")) for c in calls) / 1000, 1),
        "api_errors": len(t.raws("api_error")) + sum(bool(c.get("error")) for c in calls),
        "tool_rejections": sum(e.get("decision") == "reject" for e in t.raws("tool_decision")),
    }


def model_calls(payloads: list[dict[str, Any]], usage: Usage) -> list[dict[str, Any]]:
    """Model call events in GitPrompt's names (spec `usage`), input tokens without cached ones."""
    calls = [{name: pluck(p, path) for name, path in usage.values.items()} for p in payloads]
    if usage.input_includes_cached:
        for c in calls:
            c["input_tokens"] = max(0.0, _num(c.get("input_tokens")) - _num(c.get("cache_read_tokens")))
    return calls


def _by(payloads: list[dict[str, Any]], key: str) -> dict[Any, list[dict[str, Any]]]:
    out: dict[Any, list[dict[str, Any]]] = {}
    for e in payloads:
        out.setdefault(e.get(key), []).append(e)
    return out


# Who a model call worked for, from telemetry `api_request.query_source` (Claude Code's values):
# the main thread answering you, a subagent (`agent:builtin:Explore`...), or a system call the tool
# makes on its own (compaction, titles, summaries, suggestions, hook prompts...).
MAIN_QUERY_SOURCES = {None, "repl_main_thread", "sdk"}
SUBAGENT_PREFIX = "agent:"
WORK_KINDS = ("main", "subagent", "system")


def work_kind(query_source: str | None) -> str:
    """main, subagent or system."""
    if query_source in MAIN_QUERY_SOURCES:
        return "main"
    return "subagent" if str(query_source).startswith(SUBAGENT_PREFIX) else "system"


def _usage_of(calls: list[dict[str, Any]]) -> dict[str, Any]:
    """Requests, tokens and cost of some model calls."""
    return {
        "api_requests": len(calls),
        **{k: int(sum(_num(c.get(k)) for c in calls)) for k in USAGE_TOKENS},
        "cost_usd": round(sum(_num(c.get("cost_usd")) for c in calls), 4),
    }


def turn(prompt: dict[str, Any], events: list[dict[str, Any]], spec: Spec | None = None) -> dict[str, Any]:
    """What happened while the tool worked on one prompt. Without a spec every tool is "other" and
    no event counts as a model call."""
    if not spec:
        return TURN.derive(Turn(prompt, events))
    return TURN.derive(Turn(prompt, events, spec.tool_kinds, spec.tool_files, spec.usage))


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


@SESSION(
    {
        "starts": "how the session started, then each time it was taken up again (SessionStart.source: startup, resume...)",
        "ended / instructions_loaded / cwd_changes / config_changes": "end reason, instruction files, directory and config changes",
    }
)
def _lifecycle(events: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "starts": [r["source"] for r in _raws(events, "SessionStart") if r.get("source")],
        "ended": next((r.get("reason") for r in reversed(_raws(events, "SessionEnd"))), None),
        "instructions_loaded": sorted({r.get("file_path") for r in _raws(events, "InstructionsLoaded")} - {None}),
        "cwd_changes": [r.get("new_cwd") for r in _raws(events, "CwdChanged")],
        "config_changes": [r.get("file_path") or r.get("source") for r in _raws(events, "ConfigChange")],
    }


@SESSION({"subagent_results": "each subagent that finished: its type, how it stopped and its last message (SubagentStop)"})
def _subagents(events: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "subagent_results": [
            compact(
                {
                    "agent_id": r.get("agent_id"),
                    "agent_type": r.get("agent_type"),
                    "stop_reason": r.get("stop_reason"),
                    "answer": _text(r.get("last_assistant_message")),
                }
            )
            for r in _raws(events, "SubagentStop")
        ]
    }


def metric(event: dict[str, Any]) -> tuple[str, float] | None:
    """(name, value) of a telemetry metric data point, e.g. claude_code.lines_of_code.count[added]."""
    raw = event["raw"]
    if not (event["event"] or "").startswith(METRIC_PREFIX) or "value" not in raw:
        return None
    return event["event"] + (f"[{raw['type']}]" if raw.get("type") else ""), _num(raw["value"])


# Slash commands in telemetry: a `user_prompt` event with a `command_name`. Only the ones that expand
# into a prompt reach the hooks, so the others are counted per session.
COMMAND_EVENT = "user_prompt"


def command(event: dict[str, Any]) -> tuple[str, str | None] | None:
    """(/command, its source: builtin, custom...) of a telemetry slash command event."""
    raw = event["raw"]
    if event["event"] != COMMAND_EVENT or not raw.get("command_name"):
        return None
    return "/" + str(raw["command_name"]).lstrip("/"), raw.get("command_source")


@SESSION({"metric_totals": "telemetry metric totals: lines of code, commits, active time, tokens, cost"})
def _metrics(events: list[dict[str, Any]]) -> dict[str, Any]:
    totals: Counter[str] = Counter()
    for name, value in filter(None, map(metric, events)):
        totals[name] += value
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
    return row | origin.classify(row, events, spec) | {"stats": prompt_stats(row["prompt"] or ""), "summary": turn(row, events, spec)}


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
