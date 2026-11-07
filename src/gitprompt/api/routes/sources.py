"""Detect sources and connect/disconnect them with explicit consent (dry run shows the exact diff).

Each source is reported with raw facts only (detected, connected, policy, counts, activity per hour,
what its spec collects); the dashboard derives the connector state from them (web/src/lib/sources/state.ts).
"""

import asyncio
from datetime import UTC, datetime, timedelta

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from gitprompt.api.deps import ConnectionDep, IndexDep, SettingsDep, SpecDep, SpecsDep, StoreDep, require_dashboard
from gitprompt.core.config import Settings
from gitprompt.core.payload import extract
from gitprompt.sources import probe
from gitprompt.sources.connection import Change, Connection
from gitprompt.sources.spec import Spec
from gitprompt.storage.store import Store

router = APIRouter(prefix="/api/sources", tags=["sources"])
writes = APIRouter(prefix="/api/sources/{source}", tags=["sources"], dependencies=[Depends(require_dashboard)])

HOURS = 24  # the heartbeat window: prompts and events per hour, the current hour last


class SourceOut(BaseModel):
    id: str
    name: str
    part_of: str | None = None  # the tool's main connector, when this one is an optional layer of it
    layer: str = ""  # e.g. "Hooks", "Telemetry"
    summary: str = ""  # what this layer adds
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


class Remapped(BaseModel):
    updated: int


def hourly(store: Store, now: datetime) -> dict[str, list[int]]:
    """Per source: rows received in each of the last HOURS UTC hours, the current hour last."""
    start = now.astimezone(UTC).replace(minute=0, second=0, microsecond=0) - timedelta(hours=HOURS - 1)
    slots = {(start + timedelta(hours=i)).strftime("%Y-%m-%dT%H"): i for i in range(HOURS)}
    out: dict[str, list[int]] = {}
    for source, hour, n in store.hourly(start.isoformat()):
        if hour in slots:
            out.setdefault(source, [0] * HOURS)[slots[hour]] += n
    return out


async def describe(spec: Spec, settings: Settings, stats: dict, activity: dict[str, list[int]]) -> SourceOut:
    connection = Connection(spec, settings)
    policy = probe.check_policy(spec, settings)
    return SourceOut(
        id=spec.id,
        name=spec.name,
        part_of=spec.part_of,
        layer=spec.layer,
        summary=spec.summary,
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
    )


@router.get("")
async def list_sources(specs: SpecsDep, settings: SettingsDep, store: StoreDep) -> list[SourceOut]:
    stats, activity = store.stats(), hourly(store, datetime.now(UTC))
    return await asyncio.gather(*(describe(s, settings, stats, activity) for s in specs.values()))


@writes.post("/connect")
async def connect(connection: ConnectionDep, dry_run: bool = False) -> Change:
    return connection.connect(dry_run)


@writes.post("/disconnect")
async def disconnect(connection: ConnectionDep, dry_run: bool = False) -> Change:
    return connection.disconnect(dry_run)


@writes.post("/remap")
async def remap(spec: SpecDep, store: StoreDep, index: IndexDep) -> Remapped:
    """Rebuild mapped fields from stored raw payloads, e.g. after editing the spec, then the index."""
    updated = store.remap(spec.id, lambda raw: extract(raw, spec.map))
    index.rebuild()
    return Remapped(updated=updated)
