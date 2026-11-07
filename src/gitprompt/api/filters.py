"""Query parameters shared by the prompt list and statistics: one field per storage.store.FILTERS entry."""

from datetime import UTC, datetime
from typing import Annotated, Any, Literal

from fastapi import Query
from pydantic import BaseModel, Field, field_validator

from gitprompt.sources.spec import Origin

Outcome = Literal["done", "failed", "interrupted", "superseded", "running", "open"]  # storage.store.OUTCOMES
Has = Literal[
    "code", "url", "mention", "secret", "commit", "subagent", "tool_failure", "api_error", "denial", "compaction", "no_follow_up"
]  # storage.store.HAS


class PromptFilters(BaseModel):
    q: str | None = None  # every word appears in the typed text (full-text index)
    origin: list[Origin] = []  # empty = every origin; the dashboard sends its default (typed, slash_command)
    model: list[str] = []  # the turn used one of these models
    source: str | None = None
    session: list[str] = []  # in one of these sessions
    repo: str | None = None  # git root, or the working directory outside a repository
    since: datetime | None = None  # received at or after (ISO-8601; without an offset: local time)
    until: datetime | None = None  # received before
    weekday: Annotated[int | None, Field(ge=1, le=7)] = None  # local ISO weekday, 1 = Monday
    hour: Annotated[int | None, Field(ge=0, le=23)] = None  # local hour
    command: str | None = None  # slash command, with its slash
    tool: str | None = None  # the turn called this tool
    tool_kind: str | None = None  # ...or a tool of this kind
    query_source: str | None = None  # the turn made model calls for this purpose (telemetry)
    outcome: list[Outcome] = []  # how the turn ended
    size: str | None = None  # a `size` bucket key of /api/stats
    has: list[Has] = []  # what the prompt or its turn contains: all of them
    tag: str | None = None
    starred: bool = False
    before: int | None = None  # paging cursor: prompts with a smaller id

    @field_validator("since", "until")
    @classmethod
    def _utc(cls, value: datetime | None) -> datetime | None:
        return value and value.astimezone(UTC)

    def store_filters(self) -> dict[str, Any]:
        """As storage.store.where() takes them: instants as stored (UTC ISO-8601 with microseconds)."""
        out = self.model_dump()
        for key in ("since", "until"):
            out[key] = out[key] and out[key].isoformat(timespec="microseconds")
        return out


FiltersQuery = Annotated[PromptFilters, Query()]
