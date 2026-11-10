"""The receiver token hooks present to /ingest: when it was created, and rotating it (which
reconnects every connected source, with the same dry-run diff as Connect)."""

from datetime import UTC, datetime

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from gitprompt.api.deps import SettingsDep, SpecsDep, require_dashboard
from gitprompt.sources.connection import Connection
from gitprompt.sources.rotation import Rotation, rotate

router = APIRouter(prefix="/api/security", tags=["security"])
writes = APIRouter(prefix="/api/security", tags=["security"], dependencies=[Depends(require_dashboard)])


class SecurityOut(BaseModel):
    token_file: str
    token_changed: str  # when the token was created or last rotated
    connected: list[str]  # names of the sources that carry the token in their config


@router.get("")
async def security(settings: SettingsDep, specs: SpecsDep) -> SecurityOut:
    settings.token()
    changed = datetime.fromtimestamp(settings.token_file.stat().st_mtime, UTC).isoformat()
    connected = [s.name for s in specs.values() if Connection(s, settings).connected]
    return SecurityOut(token_file=str(settings.token_file), token_changed=changed, connected=connected)


@writes.post("/rotate-token")
async def rotate_token(settings: SettingsDep, specs: SpecsDep, dry_run: bool = True) -> Rotation:
    return rotate(specs, settings, dry_run)
