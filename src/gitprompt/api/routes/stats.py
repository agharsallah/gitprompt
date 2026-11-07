"""Statistics over every prompt the filters keep, computed in SQL on the prompt index (ADR-0012)."""

from typing import Annotated

from fastapi import APIRouter, Query
from pydantic import BaseModel

from gitprompt.api.deps import IndexDep
from gitprompt.api.filters import FiltersQuery
from gitprompt.storage.stats import GroupBy, aggregate

router = APIRouter(prefix="/api/stats", tags=["stats"])


class Bucket(BaseModel):
    """Measures of one group (or of all prompts, `key` null). Usage needs the telemetry source."""

    key: str | None  # the group's value; null = none (no model, no repository, no end...)
    prompts: int
    sessions: int
    api_requests: int
    cost_usd: float
    input_tokens: int
    output_tokens: int
    cache_read_tokens: int
    cache_creation_tokens: int
    tool_calls: int
    tool_failures: int
    ended: int  # prompts whose turn ended (Stop or StopFailure): the ones with a response time
    response_s_p50: float | None
    response_s_p90: float | None
    first_at: str | None
    last_at: str | None


class Stats(BaseModel):
    group_by: GroupBy | None
    totals: Bucket
    groups: list[Bucket]  # time buckets by key; other groupings by prompt count, largest first


@router.get("")
async def stats(
    index: IndexDep, filters: FiltersQuery, group_by: GroupBy | None = None, limit: Annotated[int, Query(ge=1, le=10_000)] = 1000
) -> Stats:
    """Counts, sums and response-time quantiles for the prompts the filters keep (the same filters as
    /api/prompts), optionally grouped. For `model`, `tool` and `tool_kind` a prompt counts in each
    group it touched; usage (model) and tool calls (tool, tool_kind) are split exactly."""
    result = aggregate(index.store.db, group_by, limit, **filters.store_filters())
    return Stats(group_by=group_by, **result)
