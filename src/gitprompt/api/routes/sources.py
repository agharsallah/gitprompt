"""Detect sources and connect/disconnect them with explicit consent (dry run shows the exact diff)."""

import asyncio

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from gitprompt.api.deps import ConnectionDep, SettingsDep, SpecDep, SpecsDep, StoreDep, require_dashboard
from gitprompt.core.config import Settings
from gitprompt.core.payload import extract
from gitprompt.sources import probe
from gitprompt.sources.connection import Change, Connection
from gitprompt.sources.spec import Spec

router = APIRouter(prefix="/api/sources", tags=["sources"])
writes = APIRouter(prefix="/api/sources/{source}", tags=["sources"], dependencies=[Depends(require_dashboard)])


class SourceOut(BaseModel):
    id: str
    name: str
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


class Remapped(BaseModel):
    updated: int


async def describe(spec: Spec, settings: Settings, stats: dict) -> SourceOut:
    connection = Connection(spec, settings)
    policy = probe.check_policy(spec, settings)
    return SourceOut(
        id=spec.id,
        name=spec.name,
        detected=probe.detected(spec, settings),
        version=await asyncio.to_thread(probe.tool_version, spec),
        connected=connection.connected,
        outdated=connection.outdated,
        config_file=str(connection.config_file),
        terms=spec.terms,
        blocks=policy.blocks,
        warnings=policy.warnings,
        **stats.get(spec.id, {}),
    )


@router.get("")
async def list_sources(specs: SpecsDep, settings: SettingsDep, store: StoreDep) -> list[SourceOut]:
    stats = store.stats()
    return await asyncio.gather(*(describe(s, settings, stats) for s in specs.values()))


@writes.post("/connect")
async def connect(connection: ConnectionDep, dry_run: bool = False) -> Change:
    return connection.connect(dry_run)


@writes.post("/disconnect")
async def disconnect(connection: ConnectionDep, dry_run: bool = False) -> Change:
    return connection.disconnect(dry_run)


@writes.post("/remap")
async def remap(spec: SpecDep, store: StoreDep) -> Remapped:
    """Rebuild mapped fields from stored raw payloads, e.g. after editing the spec."""
    return Remapped(updated=store.remap(spec.id, lambda raw: extract(raw, spec.map)))
