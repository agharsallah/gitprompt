"""User preferences (core/preferences.py): each value with where it comes from; env-set ones are locked."""

from typing import Annotated, Any

from fastapi import APIRouter, Body, Depends
from pydantic import BaseModel

from gitprompt.api.deps import PreferencesDep, require_dashboard
from gitprompt.core.preferences import APPLIES_AT_START, PreferenceFile, Resolved, Source

router = APIRouter(prefix="/api/settings", tags=["settings"])
writes = APIRouter(prefix="/api/settings", tags=["settings"], dependencies=[Depends(require_dashboard)])


class SettingOut(BaseModel):
    value: Any
    default: Any
    source: Source
    env: str  # the environment variable that would lock it
    locked: bool
    applies: str  # "now" or "restart"


class SettingsOut(BaseModel):
    file: str
    settings: dict[str, SettingOut]


class SettingsSaved(SettingsOut):
    changed: list[str]
    restart_required: bool


def _out(preferences: PreferenceFile) -> SettingsOut:
    def one(r: Resolved) -> SettingOut:
        return SettingOut(value=r.value, default=r.default, source=r.source, env=r.env, locked=r.locked, applies=r.applies)

    return SettingsOut(file=str(preferences.path), settings={k: one(r) for k, r in preferences.resolve().items()})


@router.get("")
async def get_settings(preferences: PreferencesDep) -> SettingsOut:
    return _out(preferences)


@writes.put("")
async def put_settings(preferences: PreferencesDep, changes: Annotated[dict[str, Any], Body()]) -> SettingsSaved:
    """Change some preferences (a null value resets one to its default). Refused as a whole if a
    value is invalid or set by an environment variable."""
    changed = preferences.update(changes)
    return SettingsSaved(**_out(preferences).model_dump(), changed=changed, restart_required=bool(APPLIES_AT_START & set(changed)))
