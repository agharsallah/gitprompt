"""The database and backups: where they are, how big, and destructive clean-ups that always
preview first (dry run) and only change something when confirmed (see capture/compact.py)."""

from datetime import UTC, datetime
from pathlib import Path

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from gitprompt.api.deps import SettingsDep, SpecsDep, StoreDep, require_dashboard
from gitprompt.capture import compact
from gitprompt.core.errors import GitPromptError

router = APIRouter(prefix="/api/data", tags=["data"])
writes = APIRouter(prefix="/api/data", tags=["data"], dependencies=[Depends(require_dashboard)])


class Backup(BaseModel):
    name: str
    path: str
    bytes: int
    modified: str


class SourceEvents(BaseModel):
    source: str
    rows: int
    bytes: int


class DataOut(BaseModel):
    home: str
    db_path: str
    db_bytes: int
    receiver: str
    events: list[SourceEvents]  # stored raw events per source
    backups: list[Backup]  # copies of tool config files taken before each connect/disconnect


class RemovalOut(BaseModel):
    source: str
    event: str | None
    rows: int
    bytes: int
    why: str
    before: str | None


class ReportOut(BaseModel):
    removals: list[RemovalOut]
    rows: int
    bytes: int
    size_before: int
    size_after: int | None  # set once applied
    applied: bool


class CompactIn(BaseModel):
    older_than_days: int | None = None  # also remove raw events older than this


class DeleteIn(BaseModel):
    source: str | None = None
    before: datetime | None = None  # only events received before this time


def _report(report: compact.Report) -> ReportOut:
    return ReportOut(
        removals=[RemovalOut(**vars(r)) for r in report.removals],
        rows=report.rows,
        bytes=report.bytes,
        size_before=report.size_before,
        size_after=report.size_after,
        applied=report.size_after is not None,
    )


def _backups(folder: Path) -> list[Backup]:
    files = sorted((p for p in folder.glob("*") if p.is_file()), key=lambda p: p.stat().st_mtime, reverse=True)
    return [
        Backup(name=p.name, path=str(p), bytes=p.stat().st_size, modified=datetime.fromtimestamp(p.stat().st_mtime, UTC).isoformat())
        for p in files
    ]


@router.get("")
async def data(settings: SettingsDep, store: StoreDep) -> DataOut:
    per_source: dict[str, SourceEvents] = {}
    for u in store.event_usage():
        s = per_source.setdefault(u["source"], SourceEvents(source=u["source"], rows=0, bytes=0))
        s.rows, s.bytes = s.rows + u["rows"], s.bytes + u["bytes"]
    return DataOut(
        home=str(settings.home),
        db_path=str(settings.db_path),
        db_bytes=store.size(),
        receiver=settings.receiver,
        events=sorted(per_source.values(), key=lambda s: -s.bytes),
        backups=_backups(settings.backups),
    )


@writes.post("/compact")
async def compact_now(store: StoreDep, specs: SpecsDep, body: CompactIn, dry_run: bool = True) -> ReportOut:
    """Delete stored events the specs no longer keep (and, optionally, expired ones), then VACUUM."""
    if body.older_than_days is not None and body.older_than_days < 1:
        raise GitPromptError("older_than_days must be at least 1")
    return _report(compact.compact(store, specs, body.older_than_days, apply=not dry_run))


@writes.post("/delete-events")
async def delete_events(store: StoreDep, body: DeleteIn, dry_run: bool = True) -> ReportOut:
    """Delete raw events of one source and/or received before a time, then VACUUM. Prompts and notes
    are kept. Without `before`, the dry run fixes it to now: confirm with the `before` it returns."""
    if body.source is None and body.before is None:
        raise GitPromptError("choose a source or a date: deleting every event is not offered")
    before = (body.before or datetime.now(UTC)).astimezone(UTC).isoformat()
    return _report(compact.plan(store, compact.selected(store, body.source, before), apply=not dry_run))
