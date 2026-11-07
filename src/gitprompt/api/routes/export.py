"""Exports (plan E5, ADR-0018): GET /api/export/{kind} renders a session story, a project story, the
Overview or a prompt selection to a file. The dashboard's preview and the downloaded file are the
same response, so what you see is what you send. Nothing is stored and nothing leaves the machine."""

import re
from datetime import datetime
from typing import Annotated, Any, Literal

from fastapi import APIRouter, HTTPException, Query
from fastapi.responses import Response
from pydantic import BaseModel

from gitprompt import export
from gitprompt.api.deps import IndexDep, SettingsDep
from gitprompt.api.filters import FiltersQuery, PromptFilters
from gitprompt.api.routes import stories as story_routes
from gitprompt.export.build import OVERVIEW_GROUPS
from gitprompt.insights import derivations
from gitprompt.storage import timeline
from gitprompt.storage.stats import aggregate

router = APIRouter(prefix="/api/export", tags=["export"])

Kind = Literal["session", "project", "overview", "prompts"]
FormatName = Literal["md", "html", "csv", "json", "svg"]
MAX_PROMPTS = 2000  # a prompt selection: the newest this many
GROUP_LIMIT = 400  # buckets per grouping of an overview (days of a year, projects...)


class ExportQuery(BaseModel):
    """export.Options as query parameters (defaults: the most private choice), and what to render."""

    id: str | None = None  # session: its id; project: its project id
    format: FormatName = "html"
    chart: str | None = None  # svg: the chart's id (GET .../charts), else the first one
    download: bool = False  # as an attachment, with a file name
    prompt_text: bool = False
    relative_paths: bool = True
    drop_identity: bool = True
    drop_raw: bool = True

    def options(self) -> export.Options:
        return export.Options(**{k: getattr(self, k) for k in export.Options.names()})


class ChartInfo(BaseModel):
    id: str
    title: str


async def _collect(kind: Kind, q: ExportQuery, filters: PromptFilters, index: Any) -> tuple[dict[str, Any], set[str]]:
    """The data an export shows, before redaction, and the project folders it names."""
    now = datetime.now().astimezone().isoformat()
    db = index.store.db
    if kind in ("session", "project"):
        if not q.id:
            raise HTTPException(422, f"a {kind} export needs an id")
        if kind == "session":
            story = await story_routes.session_story(q.id, index)
            environment = derivations.session(index.store.events_for_session(q.id)).get("environment")
            return {"generated_at": now, "story": story, "environment": environment}, set()
        return {"generated_at": now, "story": await story_routes.project_story(q.id, index)}, set()
    kept = filters.store_filters()
    kept.pop("before", None)
    shown = {k: v for k, v in filters.model_dump(mode="json").items() if v not in (None, [], False) and k != "before"}
    if kind == "overview":
        groups = {
            g: aggregate(db, g, GROUP_LIMIT, None, "prompts" if g in ("repo", "command") else None, **kept)["groups"]
            for g in OVERVIEW_GROUPS
        }
        totals = aggregate(db, None, **kept)["totals"]
        roots = {b["key"] for b in groups["repo"] if b.get("key")}
        return {"generated_at": now, "filters": shown, "totals": totals, "groups": groups}, roots
    rows = timeline.prompts(db, limit=MAX_PROMPTS, newest_first=True, **kept)
    if not q.drop_raw:
        raw = {r["id"]: r["raw"] for r in index.store.search(MAX_PROMPTS, **kept)}
        rows = [{**r, "raw": raw.get(r["id"])} for r in rows]
    return {"generated_at": now, "filters": shown, "prompts": rows, "total": timeline.count(db, **kept)}, set()


async def _prepare(kind: Kind, q: ExportQuery, filters: PromptFilters, index: Any, settings: Any) -> export.Export:
    data, roots = await _collect(kind, q, filters, index)
    return export.prepare(kind, data, q.options(), str(settings.user_home), roots)


def _filename(e: export.Export, extension: str) -> str:
    """gitprompt-<kind>[-<title>]-<day>.<ext>: ASCII only, safe in a header."""
    slug = re.sub(r"[^a-z0-9]+", "-", e.report.title.lower()).strip("-")[:40]
    parts = [e.report.kind, *([slug] if slug and slug != e.report.kind else []), datetime.now().strftime("%Y-%m-%d")]
    return "gitprompt-" + "-".join(parts) + extension


@router.get("/{kind}")
async def render_export(
    kind: Kind, q: Annotated[ExportQuery, Query()], filters: FiltersQuery, index: IndexDep, settings: SettingsDep
) -> Response:
    """The export as a file: Markdown, self-contained HTML, CSV or JSON of its rows, or one chart as
    SVG. Takes the export options (each defaults to the most private choice) and, for `overview` and
    `prompts`, the prompt filters."""
    e = await _prepare(kind, q, filters, index, settings)
    fmt = export.FORMATS[q.format]
    try:
        body = export.render(e, q.format, q.chart)
    except LookupError as error:
        raise HTTPException(404, str(error)) from error
    headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff"}
    if q.format == "html":
        headers["Content-Security-Policy"] = export.html.CSP
    disposition = "attachment" if q.download else "inline"
    headers["Content-Disposition"] = f'{disposition}; filename="{_filename(e, fmt.extension)}"'
    return Response(body, media_type=fmt.media_type, headers=headers)


@router.get("/{kind}/charts")
async def export_charts(
    kind: Kind, q: Annotated[ExportQuery, Query()], filters: FiltersQuery, index: IndexDep, settings: SettingsDep
) -> list[ChartInfo]:
    """The charts of an export that have data: each can be downloaded alone (format svg, chart id)."""
    e = await _prepare(kind, q, filters, index, settings)
    return [ChartInfo(id=c.id, title=c.title) for c in e.report.charts() if c.items]
