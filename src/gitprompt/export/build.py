"""From redacted data to a Report and its rows, one builder per kind of export (ADR-0018). Pure: the
API collects the data (api/routes/export.py) and export/redact.py redacts it first, so a builder
only ever sees what the options keep. A text the options dropped is simply missing here.

    BUILDERS[kind](data, options) -> (Report, rows)
"""

import json
from collections import Counter
from collections.abc import Callable, Iterable
from datetime import datetime
from typing import Any

from gitprompt.export.redact import Options
from gitprompt.export.report import Bar, Chart, Figure, Figures, Items, Paragraph, Quote, Report, Section, Table

Built = tuple[Report, list[dict[str, Any]]]
Builder = Callable[[dict[str, Any], Options], Built]
BUILDERS: dict[str, Builder] = {}

RAW_CHARS = 2000  # of a stored payload, in a document (the JSON and CSV rows have all of it)
TOP = 10  # items in a bar chart

OUTCOMES = {
    "done": "Done",
    "failed": "Failed",
    "interrupted": "Interrupted",
    "superseded": "Superseded",
    "running": "Running",
    "open": "No end seen",
}
ORIGINS = {
    "typed": "Typed",
    "slash_command": "Slash command",
    "task_notification": "Task notification",
    "agent_message": "Agent message",
    "subagent": "Subagent",
    "programmatic": "Programmatic",
    "unknown": "Unknown",
}
MARKERS = {**ORIGINS, "subagent_prompt": "Subagent prompt", "compaction": "Compaction"}


def builder(kind: str) -> Callable[[Builder], Builder]:
    def add(fn: Builder) -> Builder:
        BUILDERS[kind] = fn
        return fn

    return add


# --- reading values ----------------------------------------------------------------------------


def integer(n: float | None) -> str:
    return f"{round(n or 0):,}"


def money(usd: float | None) -> str:
    usd = usd or 0
    return f"${usd:,.2f}" if usd >= 0.01 or not usd else "<$0.01"


def reported(usd: float | None) -> str:
    """A cost the tool reported, or a dash when it reported none (not $0)."""
    return "—" if usd is None else money(usd)


def tool(facts: dict[str, Any] | None) -> str:
    """The tool that sent a prompt (insights/facts.py connector)."""
    return ((facts or {}).get("connector") or {}).get("label") or "—"


def model(facts: dict[str, Any] | None) -> str:
    return (facts or {}).get("model") or "—"


def tokens(facts: dict[str, Any] | None) -> str:
    """Tokens the tool reported, or a dash when it reported none (not 0)."""
    reported = (facts or {}).get("tokens")
    return "—" if reported is None else integer(reported["total"])


# Facts kept in the rows of a CSV or JSON export, as columns (insights/facts.py).
FACT_COLUMNS = ("model", "permission_mode", "tool_version", "api_requests")


def fact_columns(facts: dict[str, Any] | None) -> dict[str, Any]:
    facts = facts or {}
    return {
        "tool": ((facts.get("connector") or {}).get("id")),
        **{k: facts.get(k) for k in FACT_COLUMNS},
        "tokens": (facts.get("tokens") or {}).get("total"),
        "reasoning_tokens": (facts.get("tokens") or {}).get("reasoning"),
    }


def duration(seconds: float | None) -> str:
    if seconds is None:
        return "—"
    s = round(seconds)
    if s < 60:
        return f"{s} s"
    if s < 3600:
        return f"{s // 60} min"
    h, m = divmod(s // 60, 60)
    return f"{h} h {m} min" if m and h < 10 else f"{h} h"


def when(iso: str | None, date_only: bool = False) -> str:
    """An instant in local time, as read."""
    if not iso:
        return "—"
    try:
        at = datetime.fromisoformat(iso).astimezone()
    except ValueError:
        return iso
    return at.strftime("%Y-%m-%d" if date_only else "%Y-%m-%d %H:%M")


def span(first: str | None, last: str | None) -> str:
    if not first:
        return ""
    a, b = when(first), when(last)
    if not last or a == b:
        return a
    return f"{a} – {b[11:]}" if a[:10] == b[:10] else f"{a} – {b}"


def plural(n: int, word: str, many: str | None = None) -> str:
    return f"{n:,} {word if n == 1 else many or word + 's'}"


def changes(diffstat: dict[str, int] | None) -> str:
    if not diffstat:
        return "No commits"
    d = diffstat
    return f"{plural(d['commits'], 'commit')}, +{d['insertions']:,} −{d['deletions']:,} in {plural(d['files'], 'file')}"


def bars(id: str, title: str, counts: Iterable[tuple[str, float]], show: Callable[[float], str] = integer, caption: str = "") -> Chart:
    items = sorted(((label, value) for label, value in counts if value), key=lambda kv: -kv[1])
    return Chart(id, title, "bars", [Bar(label, value, show(value)) for label, value in items[:TOP]], caption)


def outcome_chart(outcomes: dict[str, int]) -> Chart:
    return bars("outcomes", "How the turns ended", ((OUTCOMES.get(k, k), v) for k, v in outcomes.items()))


def describe_filters(filters: dict[str, Any]) -> str:
    """The filters an overview or a prompt selection was made with, as read."""
    parts = []
    since, until = filters.get("since"), filters.get("until")
    if since or until:
        parts.append(f"{when(since, True) if since else 'the start'} to {when(until, True) if until else 'now'}")
    else:
        parts.append("All time")
    if origins := filters.get("origin"):
        parts.append(", ".join(ORIGINS.get(o, o) for o in origins).capitalize())
    for key, label in (("repo", "project"), ("source", "source"), ("model", "model"), ("tool", "tool"), ("tag", "tag")):
        if value := filters.get(key):
            parts.append(f"{label} {', '.join(value) if isinstance(value, list) else value}")
    if outcome := filters.get("outcome"):
        parts.append("outcome " + ", ".join(OUTCOMES.get(o, o).lower() for o in outcome))
    if has := filters.get("has"):
        parts.append("with " + ", ".join(h.replace("_", " ") for h in has))
    if filters.get("starred"):
        parts.append("starred")
    if filters.get("q"):
        parts.append(f"matching “{filters['q']}”")
    return " · ".join(parts)


def notes(options: Options, kind: str) -> list[str]:
    """What the export includes and leaves out: shown in the file itself, so its reader knows too."""
    return [
        "Includes the text you typed (clipped as in the dashboard), and system text quoted as such."
        if options.prompt_text
        else "Figures only: the text of your prompts, system messages, subagent briefs and session titles is left out.",
        "Secrets that look like keys, tokens or passwords are always replaced by [hidden …].",
        "Paths are relative to the project folder, and your home folder is shown as ~."
        if options.relative_paths
        else "Paths are absolute, as captured.",
        "Account email, user and organization ids, commit authors and email addresses are left out."
        if options.drop_identity
        else "Includes account identity from telemetry and commit authors.",
        *(
            ["Stored hook payloads are left out." if options.drop_raw else "Includes each prompt's stored hook payload."]
            if kind == "prompts"
            else []
        ),
        "Made by GitPrompt on this computer. Nothing was uploaded.",
    ]


def _report(kind: str, data: dict, title: str, subtitle: str, sections: list[Section], options: Options) -> Report:
    return Report(kind, title, subtitle, when(data.get("generated_at")), sections, notes(options, kind))


def _commit(c: dict[str, Any]) -> str:
    author = f" — {c['author']}" if c.get("author") else ""
    return f"{c.get('hash', '')[:7]} {c.get('subject', '')} (+{c.get('insertions', 0)} −{c.get('deletions', 0)}){author}"


def _tools(tools: dict[str, int]) -> str:
    return ", ".join(f"{k} {v}" for k, v in sorted(tools.items(), key=lambda kv: -kv[1])) or "—"


# --- a session ---------------------------------------------------------------------------------


@builder("session")
def session(data: dict[str, Any], options: Options) -> Built:
    story = data["story"]
    t = story["totals"]
    chapters = [i for i in story["items"] if i["kind"] == "chapter"]
    started, ended = story.get("started_at"), story.get("ended_at")
    seconds = (datetime.fromisoformat(ended) - datetime.fromisoformat(started)).total_seconds() if started and ended else None
    tools: Counter[str] = Counter()
    for c in chapters:
        tools.update(c.get("tools") or {})
    where = [p for p in ((story.get("project") or {}).get("name"), ", ".join(story.get("branches") or []), span(started, ended)) if p]

    models = ", ".join(story.get("models") or {}) or "—"
    summary: list[Any] = [
        Figures(
            [
                Figure("Tool", (story.get("connector") or {}).get("name") or story.get("source") or "—"),
                Figure("Models", models),
                Figure("Prompts you typed", integer(t["chapters"])),
                Figure("System items", integer(t["markers"])),
                Figure("Span", duration(seconds)),
                *([Figure("Cost", money(t["cost_usd"]))] if t["cost_usd"] else []),  # as on the page: no usage, no cost
                Figure("Tool calls", integer(t["tool_calls"])),
                Figure("Files written", integer(t["files_written"])),
                Figure("Retries", integer(t["retries"])),
                Figure("Changes", changes(t.get("diffstat"))),
            ]
        ),
        outcome_chart(t["outcomes"]),
        bars("tools", "Tool calls by kind", tools.items()),
    ]
    sections = [Section("Summary", summary)]
    if env := data.get("environment"):
        sections.append(Section("Environment", [Items([f"{k}: {v}" for k, v in sorted(env.items())], mono=True)], "From telemetry"))

    rows: list[dict[str, Any]] = []
    for item in story["items"]:
        if item["kind"] == "chapter":
            sections.append(_chapter(item))
            rows.append(_chapter_row(item))
        else:
            sections.append(_markers(item["items"]))
            rows += [{"kind": m["kind"], "id": m["id"], "received_at": m["received_at"], **_marker_text(m)} for m in item["items"]]
    title = story.get("title") or f"Session {story['session'][:8]}"
    return _report("session", data, title, " · ".join(where), sections, options), rows


def _chapter(c: dict[str, Any]) -> Section:
    f = c.get("facts") or {}
    facts = [
        OUTCOMES.get(c["outcome"], c["outcome"]),
        f"answered in {duration(c['response_s'])}" if c.get("response_s") is not None else "",
        f.get("model") or "",
        f"{tokens(f)} tokens" if f.get("tokens") else "",
        money(f["cost_usd"]) if f.get("cost_usd") is not None else "",
        plural(c["tool_calls"], "tool call"),
        plural(c["files_written"], "file") + " written" if c["files_written"] else "",
        f"retry of #{c['retry_of']}" if c.get("retry_of") else "",
    ]
    blocks: list[Any] = []
    if c.get("text") is not None:
        blocks.append(Quote(c["text"], "you", "clipped" if c.get("clipped") else ""))
    if c.get("files"):
        blocks.append(Items(c["files"], mono=True))
    if c.get("commits"):
        blocks.append(Items([_commit(x) for x in c["commits"]], mono=True))
    title = f"Prompt {c['turn']}" + (f" · {c['slash_command']}" if c.get("slash_command") else "")
    return Section(title, blocks, f"{when(c['received_at'])} · " + " · ".join(f for f in facts if f), level=3)


def _chapter_row(c: dict[str, Any]) -> dict[str, Any]:
    keys = ("id", "turn", "received_at", "origin", "slash_command", "branch", "outcome", "response_s", "cost_usd", "tokens")
    row = {"kind": "chapter", **{k: c.get(k) for k in keys}}
    row |= {k: c.get(k) for k in ("tool_calls", "tool_failures", "files_written", "subagents", "git_commits", "retry_of")}
    row |= {"commits": len(c.get("commits") or []), "insertions": (c.get("diffstat") or {}).get("insertions", 0)}
    row |= {"deletions": (c.get("diffstat") or {}).get("deletions", 0), "files": c.get("files") or []}
    row |= fact_columns(c.get("facts"))
    if "text" in c:
        row["text"] = c["text"]
    return row


def _marker_text(m: dict[str, Any]) -> dict[str, Any]:
    return {k: m[k] for k in ("label", "detail", "excerpt") if m.get(k)}


def _markers(markers: list[dict[str, Any]]) -> Section:
    counts = Counter(MARKERS.get(m["kind"], m["kind"]).lower() for m in markers)
    blocks: list[Any] = [Paragraph(", ".join(plural(n, k) for k, n in counts.items()) + ": not typed by you.", muted=True)]
    for m in markers:
        if m.get("excerpt"):
            blocks.append(Quote(m["excerpt"], "system", " · ".join(x for x in (MARKERS.get(m["kind"], ""), m.get("label")) if x)))
    return Section(plural(len(markers), "system item"), blocks, when(markers[0]["received_at"]), level=3)


# --- a project ---------------------------------------------------------------------------------


def phase_reason(phase: dict[str, Any]) -> str:
    if phase["reason"] == "gap":
        return f"After a pause of {duration(phase.get('gap_s'))}"
    if phase["reason"] == "branch":
        return f"On branch {phase['branches'][0]}" if phase.get("branches") else "Branch changed"
    return "Start"


def _measures(m: dict[str, Any]) -> Figures:
    return Figures(
        [
            Figure("Prompts", integer(m["prompts"])),
            Figure("Sessions", integer(m["sessions"])),
            Figure("Cost", money(m["cost_usd"])),
            Figure("Tool calls", integer(m["tool_calls"])),
            Figure("Files written", integer(m["files_written"])),
            Figure("Retries", integer(m["retries"])),
            Figure("Median length", f"{integer(m['median_chars'])} characters"),
            Figure("Changes", changes(m.get("diffstat"))),
        ]
    )


@builder("project")
def project(data: dict[str, Any], options: Options) -> Built:
    story = data["story"]
    t = story["totals"]
    sections = [
        Section(
            "Summary",
            [
                _measures(t),
                bars("phases", "Prompts per phase", ((f"Phase {p['index']}", p["prompts"]) for p in story["phases"]), caption=""),
                outcome_chart(t["outcomes"]),
                Paragraph(f"{plural(story.get('system', 0), 'prompt')} of other origins in this project are counted, not shown.", True),
            ],
        )
    ]
    rows = []
    texts = any("text" in e for p in story["phases"] for e in p["items"])
    for p in story["phases"]:
        table = Table(
            ["When", "Branch", "Tool", "Model", "Outcome", "Cost", "Tools", "Commits", *(["Prompt"] if texts else [])],
            [
                [
                    when(e["received_at"]),
                    e.get("branch") or "—",
                    tool(e.get("facts")),
                    model(e.get("facts")),
                    OUTCOMES.get(e["outcome"], e["outcome"]),
                    reported((e.get("facts") or {}).get("cost_usd")),
                    _tools(e.get("tools") or {}),
                    str(len(e.get("commits") or [])),
                    *([e.get("text", "")] if texts else []),
                ]
                for e in p["items"]
            ],
            [False, False, False, False, False, True, False, True, False],
        )
        commits = [_commit(c) for e in p["items"] for c in e.get("commits") or []]
        blocks: list[Any] = [_measures(p), table] + ([Items(commits, mono=True)] if commits else [])
        sections.append(Section(f"Phase {p['index']}", blocks, f"{phase_reason(p)} · {span(p['first_at'], p['last_at'])}"))
        for e in p["items"]:
            row = {"phase": p["index"], **{k: v for k, v in e.items() if k not in ("commits", "tools", "clipped", "facts")}}
            rows.append(row | {"tools": e.get("tools") or {}, "commits": len(e.get("commits") or [])} | fact_columns(e.get("facts")))
    subtitle = " · ".join(x for x in (story.get("repo"), span(t.get("first_at"), t.get("last_at"))) if x)
    return _report("project", data, story.get("name") or "Project", subtitle, sections, options), rows


# --- the overview ------------------------------------------------------------------------------

OVERVIEW_GROUPS = ("day", "origin", "model", "tool_kind", "outcome", "repo", "command")
BUCKET_COLUMNS = (
    "prompts", "sessions", "cost_usd", "input_tokens", "output_tokens", "tool_calls", "tool_failures", "failed",
    "files_written", "git_commits", "subagents", "response_s_p50", "response_s_p90", "first_at", "last_at",
)  # fmt: skip


def _named(group: str, b: dict[str, Any]) -> str:
    key = b.get("key")
    if key is None:
        return "None"
    if group == "origin":
        return ORIGINS.get(key, key)
    if group == "outcome":
        return OUTCOMES.get(key, key)
    return b.get("detail") or key if group == "repo" else key


@builder("overview")
def overview(data: dict[str, Any], options: Options) -> Built:
    t, g = data["totals"], data["groups"]
    tokens = (t.get("input_tokens") or 0) + (t.get("output_tokens") or 0)
    figures = Figures(
        [
            Figure("Prompts", integer(t["prompts"])),
            Figure("Sessions", integer(t["sessions"])),
            # Only some tools report usage, and fewer report cost: without any, they are left out, not zero.
            *([Figure("Cost", money(t["cost_usd"]))] if t.get("cost_usd") else []),
            *([Figure("Tokens", integer(tokens))] if t.get("api_requests") else []),
            Figure("Tool calls", integer(t["tool_calls"])),
            Figure("Failed turns", integer(t["failed"])),
            Figure("Median response", duration(t.get("response_s_p50"))),
            Figure("Commits by the agent", integer(t["git_commits"])),
        ]
    )
    by = {name: [(_named(name, b), b) for b in g.get(name, [])] for name in OVERVIEW_GROUPS}
    activity = Chart(
        "activity", "Prompts per day", "columns", [Bar(when(b["key"], True), b["prompts"], integer(b["prompts"])) for _, b in by["day"]]
    )
    sections = [
        Section("Summary", [figures, activity]),
        Section(
            "Prompt types",
            [
                bars("origins", "Who wrote the prompts", ((n, b["prompts"]) for n, b in by["origin"])),
                bars("commands", "Slash commands", ((n, b["prompts"]) for n, b in by["command"] if b.get("key"))),
            ],
        ),
        Section(
            "Models and cost",
            [
                bars("models", "Prompts by model", ((n, b["prompts"]) for n, b in by["model"] if b.get("key"))),
                bars("cost", "Cost by model", ((n, b["cost_usd"]) for n, b in by["model"] if b.get("key")), money),
            ],
        ),
        Section(
            "Agentic work",
            [
                bars("tool-kinds", "Tool calls by kind", ((n, b["tool_calls"]) for n, b in by["tool_kind"] if b.get("key"))),
                outcome_chart({b["key"]: b["prompts"] for _, b in by["outcome"] if b.get("key")}),
            ],
        ),
        Section(
            "Projects",
            [
                Table(
                    ["Project", "Prompts", "Sessions", "Cost", "Tool calls"],
                    [
                        [n, integer(b["prompts"]), integer(b["sessions"]), money(b["cost_usd"]), integer(b["tool_calls"])]
                        for n, b in by["repo"]
                    ],
                    [False, True, True, True, True],
                )
            ],
        ),
    ]
    rows = [{"grouping": "total", "key": None, **{c: t.get(c) for c in BUCKET_COLUMNS}}]
    rows += [
        {"grouping": name, "key": b.get("key"), "name": n, **{c: b.get(c) for c in BUCKET_COLUMNS}}
        for name in OVERVIEW_GROUPS
        for n, b in by[name]
    ]
    return _report("overview", data, "Overview", describe_filters(data.get("filters") or {}), sections, options), rows


# --- a prompt selection ------------------------------------------------------------------------

PROMPT_COLUMNS = (
    "id", "received_at", "source", "session", "repo", "branch", "origin", "slash_command", "outcome", "chars", "cost_usd",
    "input_tokens", "output_tokens", "response_s", "tool_calls", "tool_failures", "subagents", "files_written", "git_commits",
)  # fmt: skip


@builder("prompts")
def prompts(data: dict[str, Any], options: Options) -> Built:
    items = data["prompts"]
    texts = any("typed_text" in p for p in items)
    outcomes = Counter(p["outcome"] for p in items)
    origins = Counter(ORIGINS.get(p["origin"], p["origin"]) for p in items)
    shown = f"{plural(len(items), 'prompt')}" + (f" of {integer(data['total'])}, the newest" if data.get("total", 0) > len(items) else "")
    summary = Section(
        "Summary",
        [
            Figures(
                [
                    Figure("Prompts", shown),
                    Figure("Sessions", integer(len({p["session"] for p in items if p.get("session")}))),
                    *([Figure("Cost", money(cost))] if (cost := sum(p.get("cost_usd") or 0 for p in items)) else []),
                    Figure("Tool calls", integer(sum(p.get("tool_calls") or 0 for p in items))),
                ]
            ),
            outcome_chart(dict(outcomes)),
            bars("origins", "Who wrote the prompts", origins.items()),
        ],
    )
    table = Table(
        ["When", "Project", "Tool", "Model", "Origin", "Outcome", "Tokens", "Cost", "Tool calls", *(["Prompt"] if texts else [])],
        [
            [
                when(p["received_at"]),
                p.get("repo_name") or p.get("repo") or "—",
                tool(p.get("facts")),
                model(p.get("facts")),
                ORIGINS.get(p["origin"], p["origin"]),
                OUTCOMES.get(p["outcome"], p["outcome"]),
                tokens(p.get("facts")),
                reported((p.get("facts") or {}).get("cost_usd")),
                integer(p.get("tool_calls")),
                *([p.get("typed_text") or ""] if texts else []),
            ]
            for p in items
        ],
        [False, False, False, False, False, False, True, True, True, False],
    )
    sections = [summary, Section("Prompts", [table])]
    raw = [p for p in items if p.get("raw")]
    if raw:
        dumps = [f"#{p['id']} {json.dumps(p['raw'], ensure_ascii=False)[:RAW_CHARS]}" for p in raw]
        sections.append(Section("Stored payloads", [Items(dumps, mono=True)]))
    rows = []
    for p in items:
        row = {k: p.get(k) for k in PROMPT_COLUMNS} | fact_columns(p.get("facts"))
        row |= {k: p[k] for k in ("typed_text", "raw") if k in p}
        rows.append(row)
    subtitle = describe_filters(data.get("filters") or {})
    return _report("prompts", data, "Prompts", subtitle, sections, options), rows
