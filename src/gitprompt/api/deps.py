"""FastAPI dependencies: shared app state and request guards."""

import secrets
from typing import Annotated

from fastapi import Depends, Header, HTTPException, Request

from gitprompt.api.bus import Bus
from gitprompt.core.config import Settings
from gitprompt.core.errors import NotFound
from gitprompt.insights.indexer import Indexer
from gitprompt.sources.connection import Connection
from gitprompt.sources.spec import Spec, load_specs
from gitprompt.storage.store import Store


def get_settings(request: Request) -> Settings:
    return request.app.state.settings


def get_store(request: Request) -> Store:
    return request.app.state.store


def get_bus(request: Request) -> Bus:
    return request.app.state.bus


def get_specs(settings: Annotated[Settings, Depends(get_settings)]) -> dict[str, Spec]:
    return load_specs(settings)  # re-read each time: editing a spec needs no restart


def get_spec(source: str, specs: Annotated[dict[str, Spec], Depends(get_specs)]) -> Spec:
    if source not in specs:
        raise NotFound(f"unknown source {source!r}")
    return specs[source]


async def get_index(store: Annotated[Store, Depends(get_store)], specs: Annotated[dict[str, Spec], Depends(get_specs)]) -> Indexer:
    """The prompt index, brought up to date first (a rules change rebuilds it). Async on purpose: it
    runs on the event loop like ingest, so the two never interleave."""
    indexer = Indexer(store, specs)
    indexer.ensure()
    return indexer


def get_connection(spec: Annotated[Spec, Depends(get_spec)], settings: Annotated[Settings, Depends(get_settings)]) -> Connection:
    return Connection(spec, settings)


def require_token(settings: Annotated[Settings, Depends(get_settings)], authorization: Annotated[str, Header()] = "") -> None:
    if not secrets.compare_digest(authorization, f"Bearer {settings.token()}"):
        raise HTTPException(401, "missing or wrong token")


def require_dashboard(x_gitprompt: Annotated[str | None, Header()] = None) -> None:
    """Writes must carry a custom header: browsers only send it from our own origin (CORS is off)."""
    if x_gitprompt != "1":
        raise HTTPException(403, "missing X-GitPrompt header")


SettingsDep = Annotated[Settings, Depends(get_settings)]
StoreDep = Annotated[Store, Depends(get_store)]
BusDep = Annotated[Bus, Depends(get_bus)]
SpecDep = Annotated[Spec, Depends(get_spec)]
SpecsDep = Annotated[dict[str, Spec], Depends(get_specs)]
IndexDep = Annotated[Indexer, Depends(get_index)]
ConnectionDep = Annotated[Connection, Depends(get_connection)]
