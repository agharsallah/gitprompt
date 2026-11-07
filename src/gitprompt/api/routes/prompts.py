import asyncio
from collections import defaultdict
from collections.abc import AsyncIterable
from typing import Annotated, Any, Literal

from fastapi import APIRouter, Depends, Query
from fastapi.sse import EventSourceResponse
from pydantic import BaseModel

from gitprompt.api.deps import BusDep, IndexDep, SpecsDep, StoreDep, require_dashboard
from gitprompt.api.filters import FiltersQuery, Outcome
from gitprompt.core.errors import NotFound
from gitprompt.core.git import commits
from gitprompt.insights import derivations
from gitprompt.insights.indexer import prompt_facts
from gitprompt.insights.redaction import redact
from gitprompt.sources.spec import Origin, Spec
from gitprompt.storage.store import Store

router = APIRouter(prefix="/api/prompts", tags=["prompts"])
writes = APIRouter(prefix="/api/prompts/{id}", tags=["prompts"], dependencies=[Depends(require_dashboard)])


class Annotation(BaseModel):
    starred: bool = False
    rating: Literal[-1, 0, 1] = 0
    tags: list[str] = []
    note: str = ""


class Injected(BaseModel):
    kind: str  # the block's tag, e.g. ide_selection
    chars: int


class Connector(BaseModel):
    id: str  # the spec id (`source`)
    name: str  # e.g. "OpenAI Codex CLI"
    label: str  # the name on a badge, e.g. "Codex" (spec `short_name`)
    assistant: str  # who answers, e.g. "Codex"


class Tokens(BaseModel):
    input: int  # not read from the cache
    output: int
    cache_read: int
    cache_creation: int
    reasoning: int  # part of output
    total: int


class Facts(BaseModel):
    """What every page shows about a turn, computed once (insights/facts.py, ADR-0021)."""

    connector: Connector
    model: str | None  # the main model
    models: list[str] = []
    model_source: Literal["usage", "hook"] | None  # from model calls, or the model the hooks reported
    tokens: Tokens | None  # null: no model call reported (the tool can't, or not in this version)
    cost_usd: float | None  # null: no model call reported a cost
    api_requests: int
    outcome: Outcome | None
    permission_mode: str | None
    tool_version: str | None
    response_s: float | None
    answered: bool
    tool_calls: int
    tool_failures: int
    files_written: int
    git_commits: int
    subagents: int


class PromptOut(BaseModel):
    id: int
    source: str
    received_at: str
    prompt: str | None  # as received, including injected context
    origin: Origin  # who produced the text (insights/origin.py)
    typed_text: str | None  # the prompt without injected context
    injected: list[Injected]
    fields: dict[str, Any]  # mapped by the source spec
    context: dict[str, Any]  # observed at ingest: git state, tool version
    annotation: Annotation
    stats: dict[str, Any]  # derived from the prompt text (derivations.PROMPT_STATS)
    summary: dict[str, Any]  # derived from the prompt's events (derivations.TURN)
    facts: Facts  # what every page shows about the turn (insights/facts.py)


class PromptDetail(PromptOut):
    raw: dict[str, Any]
    session: dict[str, Any]  # derivations.SESSION
    commits: dict[str, Any]
    events: list[dict[str, Any]]


class Updated(BaseModel):
    updated: int


def get_row(id: int, store: StoreDep) -> dict:
    if not (row := store.get(id)):
        raise NotFound("prompt not found")
    return row


Row = Annotated[dict, Depends(get_row)]


@router.get("")
async def list_prompts(
    _index: IndexDep, store: StoreDep, specs: SpecsDep, filters: FiltersQuery, limit: Annotated[int, Query(ge=1, le=200)] = 50
) -> list[PromptOut]:
    """Newest first. Filters are applied in SQL on the prompt index (storage/index.py)."""
    rows = store.search(limit, **filters.store_filters())
    by_prompt: defaultdict[str, list[dict]] = defaultdict(list)
    for e in store.events_for_prompts([pid for r in rows if (pid := r["fields"].get("prompt_id"))]):
        by_prompt[e["prompt_id"]].append(e)
    outcomes = store.outcomes([r["id"] for r in rows])
    return [_derive(r, by_prompt[r["fields"].get("prompt_id")], specs, outcomes.get(r["id"])) for r in rows]


def _derive(row: dict, events: list[dict], specs: dict[str, Spec], outcome: str | None) -> dict[str, Any]:
    """Everything derived about a prompt, and its facts."""
    spec = specs.get(row["source"])
    derived = derivations.derive(row, events, spec)
    return derived | {"facts": prompt_facts(derived, spec, outcome)}


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
async def get_prompt(_index: IndexDep, row: Row, store: StoreDep, specs: SpecsDep) -> PromptDetail:
    pid = row["fields"].get("prompt_id")
    events = store.events_for_prompts([pid]) if pid else []
    derived = _derive(row, events, specs, store.outcomes([row["id"]]).get(row["id"]))
    return derived | {"events": events, "session": _session(store, row), "commits": await _commits_after(row)}


@writes.put("/annotation")
async def annotate(row: Row, annotation: Annotation, store: StoreDep) -> Annotation:
    store.annotate(row["id"], annotation.model_dump())
    return annotation


@writes.post("/redact")
async def hide_secrets(row: Row, store: StoreDep, index: IndexDep) -> Updated:
    """Hide detected secrets in the stored prompt, its raw payload and its events (and the index)."""
    updated = store.rewrite(row["id"], redact)
    index.reindex([row["id"]])
    return Updated(updated=updated)
