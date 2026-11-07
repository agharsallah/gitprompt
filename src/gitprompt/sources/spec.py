"""Source specs: everything GitPrompt knows about a tool lives in a YAML file, not in code.

Bundled specs ship in `gitprompt/sources/bundled/`. A file with the same name in `~/.gitprompt/sources/`
overrides it, so a tool update can be handled by editing data. Specs are re-read on every use.

Each file is loaded on its own (`load_spec_set`): a broken file is reported with its error and
skipped, and a broken override of a bundled spec falls back to the bundled file. One bad file never
stops capture, or a page, for any other tool (docs/adr/0017-connector-capabilities.md).

Adding a source = adding a YAML file. Adding a *kind* of source (a new payload format) = adding a
decoder in capture/pipeline.py and its name to `Format` below.
"""

import logging
import os
import re
from dataclasses import dataclass, field
from importlib.resources import files
from pathlib import Path
from typing import Annotated, Any, Literal

import yaml
from pydantic import BaseModel, ConfigDict, StringConstraints, ValidationError, model_validator

from gitprompt.core.config import Settings
from gitprompt.core.errors import SpecError
from gitprompt.core.payload import Mapping

log = logging.getLogger(__name__)

BUNDLED = files("gitprompt.sources") / "bundled"
# Shared settings every spec inherits: the field `map`, `tool_kinds`, `origin` rules, `injected` tags,
# `usage`, `shown` and `ignored` keys.
DEFAULTS = "_defaults.yaml"

Format = Literal["json", "otlp"]  # how a source's request bodies are decoded (see capture/pipeline.py DECODERS)

# Who produced a prompt's text (see insights/origin.py and docs/adr/0010-prompt-origin.md).
Origin = Literal["typed", "slash_command", "task_notification", "agent_message", "subagent", "programmatic", "unknown"]

# What a connector can feed into the dashboard (docs/sources.md "Capabilities per tool"). A page
# that needs one says "not available for <tool>" instead of showing zeros
# (web/src/lib/sources/capabilities.ts). A tool has the capabilities of its connected layers.
Capability = Literal[
    "prompt",  # prompt text on a prompt event
    "cwd",  # working directory, so prompts join their repository
    "model",  # model per turn
    "usage",  # tokens
    "cost",  # money
    "turn_end",  # an event when the answer is done: response time, how the turn ended
    "answer",  # the final answer text
    "tools",  # tool calls and their results
    "subagents",  # subagent start/stop, and markers on their events
    "origin_markers",  # signals that tell typed prompts from generated ones
    "transcript_path",  # path to the tool's own transcript
    "session_lifecycle",  # session start and end
    "session_metrics",  # per-session totals: lines of code, commits, slash commands run
]

# How events that are not prompts find their prompt (the turn they belong to):
#   prompt_id  by the mapped `prompt_id`, which the tool sends on every event
#   session    the tool sends no prompt id: GitPrompt names each prompt `<source>#<row id>`, and every
#              other event joins the latest prompt of its session received before it. Needs events
#              to arrive in order, i.e. hooks the tool runs synchronously.
Join = Literal["prompt_id", "session"]

# What a tool call does (insights/derivations.py: files read and written, shell commands, URLs).
# The dashboard reads the kind of each tool from the daemon and only decides how a kind is drawn
# (web/src/lib/insights/tools.ts). A tool the spec does not map is "other".
ToolKind = Literal["read", "write", "shell", "web", "mcp", "agent", "other"]


# What one model call reported (insights/derivations.py `_usage`): GitPrompt's name -> the path of the
# value in the event's payload.
UsageValue = Literal[
    "model",
    "input_tokens",  # input tokens not read from the cache
    "output_tokens",
    "cache_read_tokens",
    "cache_creation_tokens",
    "reasoning_tokens",  # part of the output tokens, when the tool says how many
    "cost_usd",
    "duration_ms",
    "query_source",  # who the call worked for: the main thread, a subagent, a system call
    "error",  # set when the call failed
    "status",  # how the call ended, in the tool's words (Codex: completed, failed, cancelled)
]


class Usage(BaseModel):
    """Which events report a model call each, and where their usage is. Defaults: _defaults.yaml."""

    model_config = ConfigDict(extra="forbid")

    events: list[str] = []
    values: dict[UsageValue, str] = {}  # GitPrompt's name -> payload path
    # The tool counts cached input in its input tokens (OpenAI's `input_tokens` includes
    # `cached_input_tokens`); GitPrompt's input_tokens leave them out, as Anthropic's do.
    input_includes_cached: bool = False


class Install(BaseModel):
    file: str  # config file the tool reads hooks from
    merge: dict[str, Any]  # JSON merged into it; strings may use {{receiver}}, {{source}}, {{token}}


class Policy(BaseModel):
    files: list[str] = []  # settings files (globs) that may restrict us, e.g. managed settings
    block_if: dict[str, list[Any]] = {}  # path -> values (or list members) meaning "off, do not install"
    block_if_set: list[str] = []  # paths whose value, unless it is ours, means someone else owns this setting
    warn_if_set: list[str] = []  # paths that may stop our entries from running (e.g. URL allowlists)
    warn_env: list[str] = []  # env vars that, when set where the tool starts, win over its settings file


class When(BaseModel):
    """Conditions of an origin or surface rule; every condition given must hold.

    Without `event`, `field` and `path` look at the prompt itself. With `event`, the rule needs an
    event of the same turn with that name, and `path`/`equals` look at that event's payload."""

    model_config = ConfigDict(extra="forbid")

    event: str | None = None  # an event of the turn with this name
    field: str | None = None  # a mapped field (see `map`) has a value
    path: str | None = None  # a raw payload path has a value
    equals: str | int | float | bool | None = None  # ...and that value is this one
    text_prefix: str | None = None  # the typed text (injected context removed) starts with this
    text_regex: re.Pattern[str] | None = None  # the typed text matches this (re.match, from the start)

    @model_validator(mode="after")
    def _has_a_condition(self) -> "When":
        if not self.model_fields_set - {"equals"}:
            raise ValueError("a rule needs at least one condition")
        if self.equals is not None and not self.path:
            raise ValueError("`equals` needs a `path`")
        return self


class OriginRule(BaseModel):
    model_config = ConfigDict(extra="forbid")

    when: When
    origin: Origin


Tag = Annotated[str, StringConstraints(pattern=r"^[A-Za-z][\w:-]*$")]

# Where a prompt was typed: the app or client of the tool (CLI, VS Code, a desktop app...), apart from
# the connector that delivered it (insights/surface.py, docs/adr/0022-surfaces-and-desktop-apps.md).
SurfaceId = Annotated[str, StringConstraints(pattern=r"^[a-z][a-z0-9-]*$")]


class SurfaceRule(BaseModel):
    """A prompt was typed in `surface` when `when` holds (same conditions as an origin rule)."""

    model_config = ConfigDict(extra="forbid")

    when: When
    surface: SurfaceId


class SurfaceInfo(BaseModel):
    """How a surface is named and drawn. `icon` is a name the dashboard maps to its icon set."""

    model_config = ConfigDict(extra="forbid")

    name: str
    icon: str = "app"


class Spec(BaseModel):
    id: str
    name: str
    assistant: str = ""  # who answers in the tool, e.g. "Claude"; shown next to the final answer
    short_name: str = ""  # the tool's name on a prompt's badge, e.g. "Codex"; empty = `name`
    # A tool can have several connectors, each an optional layer of the same tool (hooks, telemetry).
    part_of: str | None = None  # id of the tool's main connector; none = this is the main one
    layer: str = ""  # short name of this layer in its tool, e.g. "Hooks", "Telemetry"
    summary: str = ""  # one line: what this layer adds, shown before connecting
    detect: list[str]
    version_cmd: list[str] = []
    install: Install
    format: Format = "json"  # one JSON event per call, or OTLP/HTTP JSON batches
    prompt_events: list[str] = ["UserPromptSubmit"]  # events that create a prompt; others attach to one
    accept_events: list[str] = []  # empty = keep every event the tool sends
    drop_events: dict[str, str] = {}  # event (a trailing * matches a prefix) -> why it is not stored
    map: Mapping = {}
    policy: Policy = Policy()
    terms: str = ""  # shown before consent: why this is allowed by the tool's own docs
    after_connect: str = ""  # what the user still has to do in the tool once connected, if anything
    documented: dict[str, list[str]] = {}  # event ("*" = all) -> fields the tool's docs list
    not_captured: dict[str, str] = {}  # event -> why GitPrompt does not subscribe to it
    origin: list[OriginRule] = []  # first matching rule wins; the spec's own rules, then the shared ones
    injected: list[Tag] = []  # tags of context blocks a tool adds before or after what the user typed
    # Where its prompts were typed (first matching rule wins; no match = unknown, never guessed), the
    # names and icons of surfaces (shared ones in _defaults.yaml), the surfaces it can capture -> a
    # note, and the apps of the tool it can't capture -> why (shown on the Connectors page).
    surface: list[SurfaceRule] = []
    surfaces: dict[SurfaceId, SurfaceInfo] = {}
    captures: dict[SurfaceId, str] = {}
    cannot_capture: dict[str, str] = {}
    capabilities: list[Capability] = []  # what this connector yields (see Capability)
    # Capability -> the first tool version that yields it. An older installed version doesn't have it,
    # and pages say which version to update to (api/routes/sources.py).
    capability_since: dict[Capability, str] = {}
    join: Join = "prompt_id"  # how events find their prompt (see Join)
    tool_kinds: dict[str, ToolKind] = {}  # tool name (a trailing * matches a prefix) -> kind (see ToolKind)
    # Tool name (as above) -> pattern over the tool's text inputs whose first group is a file the call
    # touched, for tools that name files inside a text (a patch) instead of a path input.
    tool_files: dict[str, re.Pattern[str]] = {}
    usage: Usage = Usage()  # which events report model calls and their tokens (see Usage)
    # Payload keys (a dotted path covers what is under it) the dashboard reads from the raw payload
    # itself, not through a mapped field or a derivation -> where they are shown. The shared ones
    # are in _defaults.yaml; a spec adds its own.
    shown: dict[str, str] = {}
    # Payload keys the tool sends that GitPrompt deliberately does not use -> why. Every key a
    # fixture payload carries must be mapped, derived, shown or listed here
    # (tests/sources/test_field_coverage.py), so a field a tool update adds is never dropped silently.
    ignored: dict[str, str] = {}
    # Payload keys removed before anything is stored -> why (e.g. an account email nobody needs).
    discard: dict[str, str] = {}

    @model_validator(mode="after")
    def _known_surfaces(self) -> "Spec":
        named = {r.surface for r in self.surface} | self.captures.keys()
        if unknown := named - self.surfaces.keys():
            raise ValueError(f"surfaces without a name in `surfaces`: {sorted(unknown)}")
        return self

    def surface_info(self, surface: str | None) -> dict[str, str] | None:
        """{id, name, icon} of a surface, for a prompt's facts; None when unknown."""
        info = self.surfaces.get(surface) if surface else None
        return {"id": surface, "name": info.name, "icon": info.icon} if info and surface else None

    def discarding(self, payload: dict[str, Any]) -> dict[str, Any]:
        """The payload without the `discard` keys (top-level keys)."""
        return {k: v for k, v in payload.items() if k not in self.discard} if self.discard else payload

    @property
    def label(self) -> str:
        """The tool's name on a prompt's badge."""
        return self.short_name or self.name

    @property
    def marker(self) -> re.Pattern[str]:
        return marker(self.id)

    def keeps(self, event: str | None) -> bool:
        """Whether an incoming event is stored (`accept_events`, then `drop_events`)."""
        if self.accept_events and event not in self.accept_events:
            return False
        return not any(matches(pattern, event) for pattern in self.drop_events)


def by_name[T](table: dict[str, T], name: str | None) -> T | None:
    """A tool's entry in a table keyed by tool name: its exact name, else the first matching `prefix*`."""
    if name in table:
        return table[name]
    return next((value for pattern, value in table.items() if matches(pattern, name)), None)


def tool_kind(kinds: dict[str, ToolKind], name: str | None) -> ToolKind:
    """The kind of a tool (`tool_kinds`); a tool the spec does not map is "other"."""
    return by_name(kinds, name) or "other"


def _parse(name: str, text: str) -> dict[str, Any]:
    try:
        data = yaml.safe_load(text)
    except yaml.YAMLError as e:
        raise SpecError(f"source spec {name} is not valid YAML: {e}") from e
    if not isinstance(data, dict):
        raise SpecError(f"source spec {name} must be a YAML mapping")
    return data


# --- loading, one file at a time -------------------------------------------------------------

SpecOrigin = Literal["bundled", "user"]


@dataclass(frozen=True)
class SpecFile:
    """Where a spec was read from."""

    name: str  # file name, e.g. claude-code.yaml
    path: str
    origin: SpecOrigin
    overrides: bool = False  # a user file with the name of a bundled one


@dataclass(frozen=True)
class SpecProblem:
    """A spec file that can't be used, and what GitPrompt does instead."""

    file: SpecFile
    error: str
    fallback: str | None = None  # id of the bundled spec used instead, if any


@dataclass
class SpecSet:
    specs: dict[str, Spec] = field(default_factory=dict)
    files: dict[str, SpecFile] = field(default_factory=dict)  # spec id -> the file it was read from
    problems: list[SpecProblem] = field(default_factory=list)


def _candidates(settings: Settings) -> list[list[SpecFile]]:
    """Per file name, the files to try in order: the user's override first, then the bundled file."""
    bundled = sorted(p.name for p in BUNDLED.iterdir() if p.name.endswith(".yaml") and p.name != DEFAULTS)
    out: dict[str, list[SpecFile]] = {}
    if settings.user_sources.is_dir():
        for p in sorted(settings.user_sources.glob("*.yaml")):
            if p.name != DEFAULTS:  # the shared defaults are not overridable
                out[p.name] = [SpecFile(p.name, str(p), "user", overrides=p.name in bundled)]
    for name in bundled:
        out.setdefault(name, []).append(SpecFile(name, str(BUNDLED / name), "bundled"))
    return list(out.values())


def read_spec_file(file: SpecFile) -> str:
    return (BUNDLED / file.name).read_text() if file.origin == "bundled" else Path(file.path).read_text()


def parse_spec(name: str, text: str, defaults: dict[str, Any]) -> Spec:
    """One spec file on top of the shared defaults. Raises SpecError naming the file."""
    data = _parse(name, text)
    data["map"] = defaults.get("map", {}) | (data.get("map") or {})  # a spec replaces defaults per field
    data["tool_kinds"] = (defaults.get("tool_kinds") or {}) | (data.get("tool_kinds") or {})  # ...and per tool
    data["origin"] = (data.get("origin") or []) + (defaults.get("origin") or [])  # own markers first
    data["surface"] = (data.get("surface") or []) + (defaults.get("surface") or [])
    data["injected"] = list(dict.fromkeys((defaults.get("injected") or []) + (data.get("injected") or [])))
    data["usage"] = data.get("usage") or defaults.get("usage") or {}  # a spec replaces the default whole
    for key in ("shown", "ignored", "surfaces"):  # ...and adds its own keys (and surfaces) to the shared ones
        data[key] = (defaults.get(key) or {}) | (data.get(key) or {})
    try:
        return Spec(**data)
    except (ValidationError, TypeError) as e:
        raise SpecError(f"source spec {name} is invalid: {e}") from e


def _load_one(candidates: list[SpecFile], defaults: dict[str, Any], out: SpecSet) -> None:
    """The first candidate that parses wins; a failure before it is a problem (with its fallback)."""
    failed: SpecProblem | None = None
    for file in candidates:
        try:
            spec = parse_spec(file.name, read_spec_file(file), defaults)
        except (SpecError, OSError, UnicodeDecodeError) as e:
            failed = failed or SpecProblem(file, str(e))
            continue
        if failed:
            failed = SpecProblem(failed.file, failed.error, fallback=spec.id)
        if spec.id in out.specs:
            out.problems.append(SpecProblem(file, f"source spec {file.name}: id {spec.id!r} is already used by {out.files[spec.id].name}"))
        else:
            out.specs[spec.id], out.files[spec.id] = spec, file
        break
    if failed:
        out.problems.append(failed)


def load_spec_set(settings: Settings) -> SpecSet:
    """Every usable spec, the file each came from, and the files that can't be used. Never raises
    over a spec file; only a broken bundled `_defaults.yaml` (a packaging error) does."""
    defaults = _parse(DEFAULTS, (BUNDLED / DEFAULTS).read_text())
    out = SpecSet()
    for candidates in _candidates(settings):
        _load_one(candidates, defaults, out)
    for spec in list(out.specs.values()):
        if spec.part_of and (spec.part_of not in out.specs or out.specs[spec.part_of].part_of):
            file = out.files.pop(spec.id)
            del out.specs[spec.id]
            out.problems.append(SpecProblem(file, f"source spec {file.name}: part_of must name a main source spec, not {spec.part_of!r}"))
    for problem in out.problems:
        log.warning("%s%s", problem.error, f" (using the bundled {problem.fallback})" if problem.fallback else "")
    return out


def load_specs(settings: Settings) -> dict[str, Spec]:
    """The usable specs by id; broken files are skipped (`load_spec_set` says what went wrong)."""
    return load_spec_set(settings).specs


def matches(pattern: str, event: str | None) -> bool:
    """An event (or tool) name pattern: exact, or a prefix when it ends with `*` (e.g. `hook_execution_*`)."""
    if event is None:
        return False
    return event.startswith(pattern[:-1]) if pattern.endswith("*") else event == pattern


def marker(source: str) -> re.Pattern[str]:
    """Matches our receiver URL for one source (and not for a source whose id merely starts the same)."""
    return re.compile(rf"/ingest/{re.escape(source)}(?![\w-])")


def expand(path: str, settings: Settings) -> Path:
    """`${VAR:-default}` and `~` expansion, with `~` resolved against settings.user_home."""
    path = re.sub(r"\$\{(\w+):-([^}]*)\}", lambda m: os.environ.get(m[1]) or m[2], path)
    if path.startswith("~"):
        return settings.user_home / path[2:]
    return Path(path)
