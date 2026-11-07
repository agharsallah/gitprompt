"""Detect sources and connect/disconnect them with explicit consent (dry run shows the exact diff).

Each source is reported with raw facts only (detected, connected, policy, counts, activity per hour,
what its spec collects and yields, where the spec was read from); the dashboard derives the
connector state from them (web/src/lib/sources/state.ts). Spec files that can't be used are listed
by GET /api/sources/problems, so one broken file never hides the other connectors.
"""

import asyncio
from datetime import UTC, datetime, timedelta

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from gitprompt.api.deps import ConnectionDep, IndexDep, PreferencesDep, SettingsDep, SpecDep, SpecSetDep, StoreDep, require_dashboard
from gitprompt.core.config import Settings
from gitprompt.core.errors import Conflict
from gitprompt.core.payload import extract
from gitprompt.core.preferences import PreferenceFile
from gitprompt.sources import probe
from gitprompt.sources.connection import Change, Connection
from gitprompt.sources.spec import Capability, Join, Spec, SpecFile, SpecOrigin, SpecProblem, SpecSet, read_spec_file
from gitprompt.storage.store import Store

router = APIRouter(prefix="/api/sources", tags=["sources"])
writes = APIRouter(prefix="/api/sources/{source}", tags=["sources"], dependencies=[Depends(require_dashboard)])

HOURS = 24  # the heartbeat window: prompts and events per hour, the current hour last


class SpecFileOut(BaseModel):
    name: str
    path: str
    origin: SpecOrigin  # bundled with GitPrompt, or a file of the user's in ~/.gitprompt/sources/
    overrides: bool = False  # a user file replacing the bundled one of the same name


class SpecProblemOut(BaseModel):
    file: SpecFileOut
    error: str
    fallback: str | None = None  # id of the bundled spec used instead


class SourceOut(BaseModel):
    id: str
    name: str
    part_of: str | None = None  # the tool's main connector, when this one is an optional layer of it
    layer: str = ""  # e.g. "Hooks", "Telemetry"
    summary: str = ""  # what this layer adds
    enabled: bool = True  # false: turned off in GitPrompt (preference `disabled_sources`)
    detected: bool
    version: str | None
    connected: bool
    outdated: bool = False  # connected, but the spec now adds more than what is installed
    config_file: str
    terms: str
    blocks: list[str]
    warnings: list[str]
    total: int = 0
    unmapped: int = 0
    events: int = 0
    last_at: str | None = None
    hourly: list[int] = [0] * HOURS  # prompts + events received per UTC hour, oldest first
    collects: list[str] = []  # documented events GitPrompt keeps
    not_stored: dict[str, str] = {}  # event -> why it is received but dropped (spec `drop_events`)
    not_collected: dict[str, str] = {}  # event -> why GitPrompt does not subscribe to it (spec `not_captured`)
    capabilities: list[Capability] = []  # what this connector yields (spec `capabilities`)
    join: Join = "prompt_id"  # how its events find their prompt (spec `join`)
    spec: SpecFileOut  # the file this spec was read from
    spec_problem: SpecProblemOut | None = None  # the user's override of it is broken: the bundled one is used


class SpecText(BaseModel):
    file: SpecFileOut
    text: str


class Remapped(BaseModel):
    updated: int


class Enabled(BaseModel):
    enabled: bool


def file_out(file: SpecFile) -> SpecFileOut:
    return SpecFileOut(name=file.name, path=file.path, origin=file.origin, overrides=file.overrides)


def problem_out(problem: SpecProblem) -> SpecProblemOut:
    return SpecProblemOut(file=file_out(problem.file), error=problem.error, fallback=problem.fallback)


def hourly(store: Store, now: datetime) -> dict[str, list[int]]:
    """Per source: rows received in each of the last HOURS UTC hours, the current hour last."""
    start = now.astimezone(UTC).replace(minute=0, second=0, microsecond=0) - timedelta(hours=HOURS - 1)
    slots = {(start + timedelta(hours=i)).strftime("%Y-%m-%dT%H"): i for i in range(HOURS)}
    out: dict[str, list[int]] = {}
    for source, hour, n in store.hourly(start.isoformat()):
        if hour in slots:
            out.setdefault(source, [0] * HOURS)[slots[hour]] += n
    return out


async def describe(
    spec: Spec, spec_set: SpecSet, settings: Settings, disabled: list[str], stats: dict, activity: dict[str, list[int]]
) -> SourceOut:
    connection = Connection(spec, settings)
    policy = probe.check_policy(spec, settings)
    problem = next((p for p in spec_set.problems if p.fallback == spec.id), None)
    return SourceOut(
        id=spec.id,
        name=spec.name,
        part_of=spec.part_of,
        layer=spec.layer,
        summary=spec.summary,
        enabled=spec.id not in disabled,
        detected=probe.detected(spec, settings),
        version=await asyncio.to_thread(probe.tool_version, spec),
        connected=connection.connected,
        outdated=connection.outdated,
        config_file=str(connection.config_file),
        terms=spec.terms,
        blocks=policy.blocks,
        warnings=policy.warnings,
        **stats.get(spec.id, {}),
        hourly=activity.get(spec.id, [0] * HOURS),
        collects=[e for e in spec.documented if e != "*" and spec.keeps(e)],
        not_stored=spec.drop_events,
        not_collected=spec.not_captured,
        capabilities=spec.capabilities,
        join=spec.join,
        spec=file_out(spec_set.files[spec.id]),
        spec_problem=problem_out(problem) if problem else None,
    )


@router.get("")
async def list_sources(spec_set: SpecSetDep, settings: SettingsDep, store: StoreDep, preferences: PreferencesDep) -> list[SourceOut]:
    stats, activity = store.stats(), hourly(store, datetime.now(UTC))
    disabled = preferences.effective().disabled_sources
    return await asyncio.gather(*(describe(s, spec_set, settings, disabled, stats, activity) for s in spec_set.specs.values()))


@router.get("/problems")
async def problems(spec_set: SpecSetDep) -> list[SpecProblemOut]:
    """Spec files that can't be used, with their error and the bundled spec used instead (if any)."""
    return [problem_out(p) for p in spec_set.problems]


@router.get("/{source}/spec")
async def spec_text(spec: SpecDep, spec_set: SpecSetDep) -> SpecText:
    """The YAML file this connector's spec was read from, as it is on disk."""
    file = spec_set.files[spec.id]
    return SpecText(file=file_out(file), text=read_spec_file(file))


def refuse_if_disabled(spec: Spec, preferences: PreferenceFile) -> None:
    if spec.id in preferences.effective().disabled_sources:
        raise Conflict(f"{spec.name} is turned off in GitPrompt: enable it first")


@writes.post("/connect")
async def connect(connection: ConnectionDep, preferences: PreferencesDep, dry_run: bool = False) -> Change:
    refuse_if_disabled(connection.spec, preferences)
    return connection.connect(dry_run)


@writes.post("/disconnect")
async def disconnect(connection: ConnectionDep, dry_run: bool = False) -> Change:
    return connection.disconnect(dry_run)


@writes.post("/enable")
async def enable(spec: SpecDep, preferences: PreferencesDep) -> Enabled:
    return _set_enabled(spec, preferences, True)


@writes.post("/disable")
async def disable(spec: SpecDep, preferences: PreferencesDep) -> Enabled:
    """Turn a connector off in GitPrompt: what it sends is discarded until it is enabled again. Its
    entries stay in the tool's config file; Disconnect removes them."""
    return _set_enabled(spec, preferences, False)


def _set_enabled(spec: Spec, preferences: PreferenceFile, enabled: bool) -> Enabled:
    disabled = [s for s in preferences.current().disabled_sources if s != spec.id] + ([] if enabled else [spec.id])
    preferences.update({"disabled_sources": disabled or None})
    return Enabled(enabled=enabled)


@writes.post("/remap")
async def remap(spec: SpecDep, store: StoreDep, index: IndexDep) -> Remapped:
    """Rebuild mapped fields from stored raw payloads, e.g. after editing the spec, then the index."""
    updated = store.remap(spec.id, lambda raw: extract(raw, spec.map))
    index.rebuild()
    return Remapped(updated=updated)
