"""Statistics over every prompt the filters keep, computed in SQL on the prompt index (ADR-0012,
ADR-0014), and the telemetry that arrives per session."""

from typing import Annotated

from fastapi import APIRouter, HTTPException, Query
from pydantic import BaseModel

from gitprompt.api.deps import IndexDep
from gitprompt.api.filters import FiltersQuery
from gitprompt.insights.surface import catalog
from gitprompt.storage.stats import GroupBy, Order, aggregate, telemetry

router = APIRouter(prefix="/api/stats", tags=["stats"])


class Bucket(BaseModel):
    """Measures of one group (or of all prompts, `key` null). Usage needs the telemetry source."""

    key: str | None  # the group's value; null = none (no model, no repository, no end...)
    detail: str | None = None  # one more fact about the group (storage/stats.py GROUPS): a name, an excerpt, a kind
    prompts: int
    sessions: int
    api_requests: int
    cost_usd: float
    input_tokens: int
    output_tokens: int
    cache_read_tokens: int
    cache_creation_tokens: int
    api_time_s: float  # time in model calls (telemetry)
    api_errors: int
    tool_calls: int
    tool_failures: int
    tool_time_s: float  # summed tool durations: parallel calls add up
    subagent_tool_calls: int  # tool calls made by subagents
    subagents: int  # subagents started
    files_written: int  # files each turn wrote, summed over turns
    git_commits: int  # git commits the agent ran
    compactions: int
    denials: int  # tool calls the permission system denied
    rejections: int  # tool calls you rejected (telemetry)
    ended: int  # prompts whose turn ended (Stop or StopFailure): the ones with a response time
    failed: int  # turns that ended with StopFailure
    followed: int  # prompts with a later prompt in their session
    with_code: int  # prompts whose typed text has a code block
    with_url: int
    with_mention: int
    with_secret: int
    with_commit: int  # prompts whose turn made a git commit
    distinct_texts: int  # different typed texts: prompts - distinct_texts repeated an earlier text
    response_s_p50: float | None
    response_s_p90: float | None
    span_s: float | None = None  # `session` only: first prompt to the end of the last turn
    first_at: str | None
    last_at: str | None
    parts: list["Bucket"] | None = None  # with split_by: this group's measures per value of that grouping


class Stats(BaseModel):
    group_by: GroupBy | None
    split_by: GroupBy | None = None
    totals: Bucket
    groups: list[Bucket]  # ordered by `order`: by default, time and size buckets by key, others by prompt count


class Command(BaseModel):
    command: str  # with its slash
    source: str | None  # builtin, custom...
    uses: int
    sessions: int


class Telemetry(BaseModel):
    sessions: int  # sessions of the prompts the filters keep
    metrics: dict[str, float]  # metric totals, named like derivations.SESSION metric_totals
    commands: list[Command]  # slash commands run in those sessions, most used first


@router.get("")
async def stats(
    index: IndexDep,
    filters: FiltersQuery,
    group_by: GroupBy | None = None,
    split_by: GroupBy | None = None,
    order: Order | None = None,
    limit: Annotated[int, Query(ge=1, le=10_000)] = 1000,
) -> Stats:
    """Counts, sums and response-time quantiles for the prompts the filters keep (the same filters as
    /api/prompts), optionally grouped, and each group optionally split by a second grouping. For
    `model`, `tool`, `tool_kind`, `work` and `query_source` a prompt counts in each group it touched;
    usage and tool calls are split exactly. `order` sorts groups by key (ascending) or by a measure
    (descending)."""
    if split_by and not group_by:
        raise HTTPException(422, "split_by needs group_by")
    try:
        result = aggregate(index.store.db, group_by, limit, split_by, order, **filters.store_filters())
    except ValueError as e:
        raise HTTPException(422, str(e)) from e
    surfaces = catalog(index.specs.values())
    for grouping, buckets in ((group_by, result["groups"]), (split_by, [p for g in result["groups"] for p in g.get("parts") or []])):
        for b in buckets:
            if grouping == "source" and (spec := index.specs.get(b["key"])):  # named as its badge (spec `short_name`)
                b["detail"] = b.get("detail") or spec.label
            elif grouping == "surface" and (info := surfaces.get(b["key"] or "")):  # named as its label
                b["detail"] = info.name
    return Stats(group_by=group_by, split_by=split_by, **result)


@router.get("/telemetry")
async def session_telemetry(index: IndexDep, filters: FiltersQuery) -> Telemetry:
    """Telemetry that arrives per session, not per prompt (lines of code, commits, active time, slash
    commands the hooks never see), summed over the sessions of the prompts the filters keep."""
    return Telemetry(**telemetry(index.store.db, **filters.store_filters()))
