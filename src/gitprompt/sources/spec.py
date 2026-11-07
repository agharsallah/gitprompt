"""Source specs: everything GitPrompt knows about a tool lives in a YAML file, not in code.

Bundled specs ship in `gitprompt/sources/bundled/`. A file with the same name in `~/.gitprompt/sources/`
overrides it, so a tool update can be handled by editing data. Specs are re-read on every use.

Adding a source = adding a YAML file. Adding a *kind* of source (a new payload format) = adding a
decoder in capture/pipeline.py and its name to `Format` below.
"""

import os
import re
from importlib.resources import files
from pathlib import Path
from typing import Annotated, Any, Literal

import yaml
from pydantic import BaseModel, ConfigDict, StringConstraints, ValidationError, model_validator

from gitprompt.core.config import Settings
from gitprompt.core.errors import SpecError
from gitprompt.core.payload import Mapping

BUNDLED = files("gitprompt.sources") / "bundled"
DEFAULTS = "_defaults.yaml"  # shared settings every spec inherits: the field `map`, `origin` rules, `injected` tags

Format = Literal["json", "otlp"]  # how a source's request bodies are decoded (see capture/pipeline.py DECODERS)

# Who produced a prompt's text (see insights/origin.py and docs/adr/0010-prompt-origin.md).
Origin = Literal["typed", "slash_command", "task_notification", "agent_message", "subagent", "programmatic", "unknown"]


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
    """Conditions of an origin rule; every condition given must hold.

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
            raise ValueError("an origin rule needs at least one condition")
        if self.equals is not None and not self.path:
            raise ValueError("`equals` needs a `path`")
        return self


class OriginRule(BaseModel):
    model_config = ConfigDict(extra="forbid")

    when: When
    origin: Origin


Tag = Annotated[str, StringConstraints(pattern=r"^[A-Za-z][\w:-]*$")]


class Spec(BaseModel):
    id: str
    name: str
    detect: list[str]
    version_cmd: list[str] = []
    install: Install
    format: Format = "json"  # one JSON event per call, or OTLP/HTTP JSON batches
    prompt_events: list[str] = ["UserPromptSubmit"]  # events that create a prompt; others attach to one
    accept_events: list[str] = []  # empty = keep every event the tool sends
    map: Mapping = {}
    policy: Policy = Policy()
    terms: str = ""  # shown before consent: why this is allowed by the tool's own docs
    documented: dict[str, list[str]] = {}  # event ("*" = all) -> fields the tool's docs list
    not_captured: dict[str, str] = {}  # event -> why GitPrompt does not subscribe to it
    origin: list[OriginRule] = []  # first matching rule wins; the spec's own rules, then the shared ones
    injected: list[Tag] = []  # tags of context blocks a tool adds before or after what the user typed

    @property
    def marker(self) -> re.Pattern[str]:
        return marker(self.id)


def _parse(name: str, text: str) -> dict[str, Any]:
    try:
        data = yaml.safe_load(text)
    except yaml.YAMLError as e:
        raise SpecError(f"source spec {name} is not valid YAML: {e}") from e
    if not isinstance(data, dict):
        raise SpecError(f"source spec {name} must be a YAML mapping")
    return data


def _spec_files(settings: Settings) -> dict[str, str]:
    """File name -> text. A user file replaces the bundled file with the same name."""
    texts = {p.name: p.read_text() for p in BUNDLED.iterdir() if p.name.endswith(".yaml")}
    if settings.user_sources.is_dir():
        texts |= {p.name: p.read_text() for p in settings.user_sources.glob("*.yaml")}
    texts.pop(DEFAULTS, None)
    return texts


def load_specs(settings: Settings) -> dict[str, Spec]:
    defaults = _parse(DEFAULTS, (BUNDLED / DEFAULTS).read_text())
    specs: dict[str, Spec] = {}
    for name, text in _spec_files(settings).items():
        data = _parse(name, text)
        data["map"] = defaults.get("map", {}) | (data.get("map") or {})  # a spec replaces defaults per field
        data["origin"] = (data.get("origin") or []) + (defaults.get("origin") or [])  # own markers first
        data["injected"] = list(dict.fromkeys((defaults.get("injected") or []) + (data.get("injected") or [])))
        try:
            spec = Spec(**data)
        except ValidationError as e:
            raise SpecError(f"source spec {name} is invalid: {e}") from e
        specs[spec.id] = spec
    return specs


def marker(source: str) -> re.Pattern[str]:
    """Matches our receiver URL for one source (and not for a source whose id merely starts the same)."""
    return re.compile(rf"/ingest/{re.escape(source)}(?![\w-])")


def expand(path: str, settings: Settings) -> Path:
    """`${VAR:-default}` and `~` expansion, with `~` resolved against settings.user_home."""
    path = re.sub(r"\$\{(\w+):-([^}]*)\}", lambda m: os.environ.get(m[1]) or m[2], path)
    if path.startswith("~"):
        return settings.user_home / path[2:]
    return Path(path)
