"""Where a prompt was typed: the app or client of its tool (CLI, VS Code, a desktop app...), apart
from the connector that delivered it. Derived on read from the source spec's `surface` rules
(docs/adr/0022-surfaces-and-desktop-apps.md), so a new rule applies to old prompts too.

    surface_of(prompt, events, spec) -> surface id, or None when no rule matches
    signals(prompt, events, spec)    -> the values the rules look at, to show why (gitprompt check)

A rule has the conditions of an origin rule (insights/origin.py `matches`): a path of the prompt's
payload, or of an event of its turn (Claude Code's telemetry `user_prompt` carries
`app.entrypoint`). Unknown stays None: a surface is never guessed. Pure functions, no tool knowledge.
"""

from collections.abc import Iterable
from typing import Any

from gitprompt.core.payload import NO_VALUE, pluck
from gitprompt.insights.origin import matches
from gitprompt.sources.spec import Spec, SurfaceInfo

DOCS = {
    "surface": "where the prompt was typed: the app or client of its tool (cli, vscode, desktop...); null when unknown",
}


def surface_of(prompt: dict[str, Any], events: list[dict[str, Any]], spec: Spec | None) -> str | None:
    """The first surface rule of the spec that holds for this prompt and its turn, else None."""
    if spec is None:
        return None
    return next((r.surface for r in spec.surface if matches(r.when, prompt, events, "")), None)


def signals(prompt: dict[str, Any], events: list[dict[str, Any]], spec: Spec | None) -> dict[str, Any]:
    """Each payload path the surface rules read -> the value this prompt (or its turn) has there.
    Shows why a prompt got its surface, or what an unknown client reported."""
    out: dict[str, Any] = {}
    for rule in spec.surface if spec else []:
        path, event = rule.when.path, rule.when.event
        if not path or path in out:
            continue
        payloads = [e["raw"] for e in events if e["event"] == event] if event else [prompt.get("raw") or {}]
        value = next((v for p in payloads if (v := pluck(p, path)) not in NO_VALUE), None)
        if value is not None:
            out[path] = value
    return out


def catalog(specs: Iterable[Spec]) -> dict[str, SurfaceInfo]:
    """Every surface any spec names (the shared ones of _defaults.yaml, and each spec's own)."""
    out: dict[str, SurfaceInfo] = {}
    for spec in specs:
        out |= spec.surfaces
    return out
