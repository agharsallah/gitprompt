"""Prompt stories and search (plan E4, ADR-0016): a session as chapters, a project as phases, a
prompt's versions, and full-text search with snippets. Data comes from the prompt index
(storage/timeline.py) and one `git log` per story; insights/stories.py shapes it."""

import asyncio
from typing import Annotated, Any, Literal

from fastapi import APIRouter, Query
from pydantic import BaseModel

from gitprompt.api.deps import IndexDep
from gitprompt.api.filters import FiltersQuery, Outcome
from gitprompt.core.errors import NotFound
from gitprompt.core.git import history
from gitprompt.insights import stories
from gitprompt.insights.snippets import Segment, highlight
from gitprompt.sources.spec import Origin
from gitprompt.storage import timeline

router = APIRouter(prefix="/api", tags=["stories"])

# Prompts compared when looking for the versions of one (the newest first). Bounds a lineage
# request on a very large history; the local data has a few hundred typed prompts.
LINEAGE_CANDIDATES = 5000


class Commit(BaseModel):
    hash: str
    subject: str
    author: str | None = None
    time: str
    files: int = 0
    insertions: int = 0
    deletions: int = 0


class Diffstat(BaseModel):
    commits: int
    files: int
    insertions: int
    deletions: int


class Project(BaseModel):
    id: str  # stories.project_id: base64url of the git root (or folder)
    repo: str
    name: str
    git: bool


class Chapter(BaseModel):
    """A prompt you typed, and what its turn amounted to."""

    kind: Literal["chapter"] = "chapter"
    id: int
    turn: int  # position in the session, counting every prompt
    received_at: str
    origin: Origin
    text: str  # the typed text (clipped at stories.CHAPTER_CHARS: the prompt detail has it all)
    clipped: bool
    slash_command: str | None
    branch: str | None
    outcome: Outcome
    response_s: float | None
    cost_usd: float
    tokens: int
    tool_calls: int
    tool_failures: int
    tools: dict[str, int]  # tool calls by kind
    files_written: int
    files: list[str]  # the first stories.MAX_FILES files written
    subagents: int
    git_commits: int  # git commits the agent ran
    commits: list[Commit]  # commits made after the prompt (stories.attribute_commits)
    diffstat: Diffstat | None
    retry_of: int | None  # an earlier chapter this one rewords or repeats


class Marker(BaseModel):
    """Something that happened in the session that you did not type."""

    kind: Literal["task_notification", "agent_message", "subagent", "programmatic", "unknown", "subagent_prompt", "compaction"]
    id: int  # the prompt to open: the system prompt itself, or the turn the subagent or compaction belongs to
    received_at: str
    label: str | None  # subagent_prompt: the task's description
    detail: str | None  # subagent_prompt: the subagent type
    excerpt: str | None  # the system text, never shown as typed
    clipped: bool


class Markers(BaseModel):
    kind: Literal["markers"] = "markers"
    items: list[Marker]


class SessionTotals(BaseModel):
    chapters: int
    markers: int
    cost_usd: float
    tool_calls: int
    files_written: int  # distinct files written by the chapters' turns
    outcomes: dict[str, int]
    retries: int
    diffstat: Diffstat | None


class SessionStory(BaseModel):
    session: str
    title: str | None
    source: str | None
    project: Project | None
    branches: list[str]
    started_at: str | None
    ended_at: str | None
    totals: SessionTotals
    items: list[Chapter | Markers]


class Entry(BaseModel):
    """A prompt in a project story."""

    id: int
    received_at: str
    session: str | None
    origin: Origin
    branch: str | None
    text: str  # an excerpt of the typed text
    clipped: bool
    chars: int
    outcome: Outcome
    response_s: float | None
    cost_usd: float
    tool_calls: int
    tool_failures: int
    tools: dict[str, int]
    files_written: int
    git_commits: int
    commits: list[Commit]
    retry_of: int | None


class Measures(BaseModel):
    prompts: int
    sessions: int
    cost_usd: float
    tool_calls: int
    files_written: int
    outcomes: dict[str, int]
    diffstat: Diffstat | None
    retries: int  # prompts that reword or repeat an earlier one of their session
    median_chars: int
    with_mention: int
    with_code: int
    first_at: str | None
    last_at: str | None


class Phase(Measures):
    index: int
    reason: Literal["start", "gap", "branch"]  # why a new phase began here
    gap_s: int | None  # reason gap: the pause before it
    branches: list[str]
    items: list[Entry]


class ProjectStory(Project):
    system: int  # prompts of other origins in the project: counted, never shown as typed
    totals: Measures
    phases: list[Phase]


class Version(BaseModel):
    id: int
    received_at: str
    session: str | None
    project: Project | None
    origin: Origin
    text: str
    clipped: bool
    similarity: float  # to the prompt asked for (insights/lineage.py)
    same_text: bool
    place: Literal["self", "session", "project", "elsewhere"]
    outcome: Outcome
    response_s: float | None
    cost_usd: float
    tool_failures: int
    diff: list[tuple[Literal["equal", "delete", "insert"], str]] | None  # from the version before
    change: Literal["improved", "worse", "same"] | None  # outcome compared with the version before


class Lineage(BaseModel):
    id: int
    threshold: float
    versions: list[Version]  # oldest first, the prompt itself included


class SearchHit(BaseModel):
    id: int
    received_at: str
    session: str | None
    title: str | None
    project: Project | None
    branch: str | None
    origin: Origin
    outcome: Outcome
    cost_usd: float
    snippet: list[Segment]  # the typed text around the first match; `hit` segments are matches


class SearchResults(BaseModel):
    total: int
    hits: list[SearchHit]  # newest first


def _log(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Commits in the story's repository while it lasted (one git call), or [] without a repository."""
    window = stories.log_window(rows)
    repo = next((p["repo"] for p in rows if p.get("branch") and p.get("repo")), None)  # a branch: it is a git repository
    return history(repo, *window) if repo and window else []


@router.get("/stories/session/{session}")
async def session_story(session: str, index: IndexDep) -> SessionStory:
    """A session as chapters (the prompts you typed) with system markers between them."""
    db = index.store.db
    rows = timeline.prompts(db, session=[session])
    if not rows:
        raise NotFound("session not found")
    user = [p for p in rows if stories.is_user(p)]
    events = timeline.turn_events(db, [p["prompt_id"] for p in rows if p.get("prompt_id")])
    commits = await asyncio.to_thread(_log, user)
    spec = index.specs.get(rows[0]["source"])  # what each tool does is the source spec's `tool_kinds`
    kinds = spec.tool_kinds if spec else {}
    return stories.session_story(session, rows, timeline.tool_kinds(db, [p["id"] for p in user]), events, commits, kinds)


@router.get("/stories/project/{project}")
async def project_story(project: str, index: IndexDep) -> ProjectStory:
    """A project's typed prompts in phases, with commits, files, cost and outcomes."""
    db = index.store.db
    repo = stories.project_repo(project)
    rows = timeline.prompts(db, repo=repo, origin=list(stories.USER_ORIGINS)) if repo else []
    if not repo or not (rows or timeline.count(db, repo=repo)):
        raise NotFound("project not found")
    system = timeline.count(db, repo=repo) - len(rows)
    commits = await asyncio.to_thread(_log, rows)
    return stories.project_story(repo, rows, timeline.tool_kinds(db, [p["id"] for p in rows]), commits, system)


@router.get("/stories/prompt/{id}")
async def prompt_lineage(id: int, index: IndexDep) -> Lineage:
    """The versions of a prompt you typed: retries in its session and the same instruction reused."""
    db = index.store.db
    if not (target := timeline.one(db, id)):
        raise NotFound("prompt not found")
    candidates = timeline.prompts(db, limit=LINEAGE_CANDIDATES, newest_first=True, origin=list(stories.USER_ORIGINS))
    return stories.prompt_lineage(target, candidates)


@router.get("/search")
async def search(index: IndexDep, filters: FiltersQuery, limit: Annotated[int, Query(ge=1, le=200)] = 50) -> SearchResults:
    """Prompts whose typed text holds every word of `q` (full-text index), newest first, each with a
    snippet around the first match. Takes the prompt filters (project, dates, origin, tags...)."""
    db = index.store.db
    kept = filters.store_filters()
    rows = timeline.prompts(db, limit=limit, newest_first=True, **kept)
    kept.pop("before", None)  # the total covers every page
    hits = [{**p, "project": stories.project_of([p]), "snippet": highlight(p.get("typed_text"), filters.q)} for p in rows]
    return SearchResults(total=timeline.count(db, **kept), hits=hits)
