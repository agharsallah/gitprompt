import asyncio
from collections import defaultdict
from collections.abc import AsyncIterable
from typing import Annotated, Any, Literal

from fastapi import APIRouter, Depends, Query
from fastapi.sse import EventSourceResponse
from pydantic import BaseModel

from gitprompt.api.deps import BusDep, StoreDep, require_dashboard
from gitprompt.core.errors import NotFound
from gitprompt.core.git import commits
from gitprompt.insights import derivations
from gitprompt.insights.redaction import redact
from gitprompt.storage.store import Store

router = APIRouter(prefix="/api/prompts", tags=["prompts"])
writes = APIRouter(prefix="/api/prompts/{id}", tags=["prompts"], dependencies=[Depends(require_dashboard)])


class Annotation(BaseModel):
    starred: bool = False
    rating: Literal[-1, 0, 1] = 0
    tags: list[str] = []
    note: str = ""


class PromptOut(BaseModel):
    id: int
    source: str
    received_at: str
    prompt: str | None
    fields: dict[str, Any]  # mapped by the source spec
    context: dict[str, Any]  # observed at ingest: git state, tool version
    annotation: Annotation
    stats: dict[str, Any]  # derived from the prompt text (derivations.PROMPT_STATS)
    summary: dict[str, Any]  # derived from the prompt's events (derivations.TURN)


class PromptDetail(PromptOut):
    raw: dict[str, Any]
    session: dict[str, Any]  # derivations.SESSION
    commits: dict[str, Any]
    events: list[dict[str, Any]]


class Updated(BaseModel):
    updated: int


def derive(row: dict, events: list[dict]) -> dict:
    """A stored prompt row plus the values derived from its text and its events."""
    return row | {"stats": derivations.prompt_stats(row["prompt"] or ""), "summary": derivations.turn(row, events)}


def get_row(id: int, store: StoreDep) -> dict:
    if not (row := store.get(id)):
        raise NotFound("prompt not found")
    return row


Row = Annotated[dict, Depends(get_row)]


@router.get("")
async def list_prompts(
    store: StoreDep,
    q: str | None = None,
    source: str | None = None,
    session: str | None = None,
    repo: str | None = None,
    tag: str | None = None,
    starred: bool = False,
    before: int | None = None,
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
) -> list[PromptOut]:
    rows = store.search(limit, q=q, source=source, session=session, repo=repo, tag=tag, starred=starred, before=before)
    by_prompt: defaultdict[str, list[dict]] = defaultdict(list)
    for e in store.events_for_prompts([pid for r in rows if (pid := r["fields"].get("prompt_id"))]):
        by_prompt[e["prompt_id"]].append(e)
    return [derive(r, by_prompt[r["fields"].get("prompt_id")]) for r in rows]


@router.get("/stream", response_class=EventSourceResponse)
async def stream(bus: BusDep) -> AsyncIterable[dict]:
    async with bus.subscribe() as queue:
        while True:
            yield await queue.get()


async def _commits_after(row: dict) -> dict[str, Any]:
    """Commits made in the prompt's repository until the next prompt of the session (or now)."""
    ctx = row["context"]
    if not ctx.get("git_root"):
        return {}
    return await asyncio.to_thread(commits, ctx["git_root"], ctx.get("git_head"), row["next_head"] or "HEAD", row["received_at"])


def _session(store: Store, row: dict) -> dict[str, Any]:
    sid = row["fields"].get("session_id")
    return derivations.session(store.events_for_session(sid)) if sid else {}


@router.get("/{id}")
async def get_prompt(row: Row, store: StoreDep) -> PromptDetail:
    pid = row["fields"].get("prompt_id")
    events = store.events_for_prompts([pid]) if pid else []
    return derive(row, events) | {"events": events, "session": _session(store, row), "commits": await _commits_after(row)}


@writes.put("/annotation")
async def annotate(row: Row, annotation: Annotation, store: StoreDep) -> Annotation:
    store.annotate(row["id"], annotation.model_dump())
    return annotation


@writes.post("/redact")
async def hide_secrets(row: Row, store: StoreDep) -> Updated:
    """Hide detected secrets in the stored prompt, its raw payload and its events."""
    return Updated(updated=store.rewrite(row["id"], redact))
