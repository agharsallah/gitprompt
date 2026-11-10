"""Recent captures: the newest prompt of each connector and surface, with how it was attributed
(session, project, model, surface) and the values the surface rules read. `gitprompt check` and the
Connectors page show it, so you can type one prompt in each app and see whether it arrived, and where
GitPrompt put it.

    recent(store, specs) -> [{connector, surface, received_at, session, project, branch, model,
                              origin, prompts, signals}]

Reads the prompt index (call Indexer.ensure() first) plus the raw prompt and turn events of each
row (for `signals`, insights/surface.py).
"""

from collections.abc import Mapping
from typing import Any

from gitprompt.insights import facts, surface
from gitprompt.sources.spec import Spec
from gitprompt.storage.stats import latest
from gitprompt.storage.store import Store


def _project(row: dict[str, Any]) -> dict[str, Any] | None:
    repo = row.get("repo")
    if not repo:
        return None
    return {"repo": repo, "name": row.get("repo_name") or repo.rstrip("/").rsplit("/", 1)[-1]}


def recent(store: Store, specs: Mapping[str, Spec]) -> list[dict[str, Any]]:
    rows = latest(store.db)
    raws = {r["id"]: r for r in store.prompts([r["id"] for r in rows])}
    turns: dict[str, list[dict[str, Any]]] = {}
    for e in store.events_for_prompts([pid for r in raws.values() if (pid := r["fields"].get("prompt_id"))]):
        turns.setdefault(e["prompt_id"], []).append(e)
    out = []
    for row in rows:
        spec, raw = specs.get(row["source"]), raws.get(row["id"], {})
        events = turns.get((raw.get("fields") or {}).get("prompt_id") or "", [])
        out.append(
            {
                "id": row["id"],
                "connector": facts.connector(row["source"], spec),
                "surface": spec.surface_info(row["surface"]) if spec else None,
                "received_at": row["received_at"],
                "session": row["session"],
                "title": row["title"],
                "project": _project(row),
                "branch": row["branch"],
                "worktree": (raw.get("context") or {}).get("git_worktree_root"),
                "model": row["model"],
                "origin": row["origin"],
                "prompts": row["prompts"],
                "signals": surface.signals(raw, events, spec) if raw else {},
            }
        )
    return out
