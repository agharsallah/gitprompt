"""Who produced a prompt, and which part of it the user actually typed. Derived on read from the
source spec's `origin` rules and `injected` tags (ADR-0010), so a changed rule reclassifies old
prompts too, with no migration.

    split_context(text, tags)     -> (typed_text, injected): leading/trailing context blocks removed
    classify(prompt, events, spec) -> {origin, typed_text, injected}

Pure functions: no I/O, no knowledge of any tool (that lives in the YAML specs).
"""

import re
from typing import Any

from gitprompt.core.payload import NO_VALUE, pluck
from gitprompt.sources.spec import Origin, OriginRule, Spec, When

DEFAULT: Origin = "typed"  # no rule matched

DOCS = {
    "origin": "who produced the prompt: typed, slash_command, task_notification, agent_message, subagent, programmatic or unknown",
    "typed_text": "the prompt without the context blocks a tool adds before or after it (IDE selection, system reminder)",
    "injected": "the context blocks removed from the typed text: their kind (tag) and size in characters",
}


def _block(tags: list[str], edge: str) -> re.Pattern[str]:
    """One `<tag ...>...</tag>` block at the start (`edge="^"`) or the end (`edge="$"`) of a text."""
    names = "|".join(map(re.escape, sorted(tags, key=len, reverse=True)))
    block = rf"<({names})(?:\s[^>]*)?>(?:(?!<\1[\s>]).)*?</\1>"
    return re.compile(rf"\A\s*{block}\s*" if edge == "^" else rf"\s*{block}\s*\Z", re.S)


def split_context(text: str, tags: list[str]) -> tuple[str, list[dict[str, Any]]]:
    """Remove the tagged blocks a tool put before or after what the user typed. Blocks in the
    middle are part of the prompt and stay. `injected` lists the removed blocks in text order."""
    if not tags:
        return text.strip(), []
    leading, trailing = [], []
    for pattern, found in ((_block(tags, "^"), leading), (_block(tags, "$"), trailing)):
        while m := pattern.search(text):
            found.append({"kind": m[1], "chars": len(m[0].strip())})
            text = text[: m.start()] + text[m.end() :]
    return text.strip(), leading + trailing[::-1]


def _value_matches(data: Any, when: When) -> bool:
    if when.path is None:
        return True
    value = pluck(data, when.path)
    return value not in NO_VALUE if when.equals is None else value == when.equals


def matches(when: When, prompt: dict[str, Any], events: list[dict[str, Any]], typed: str) -> bool:
    """Whether a rule's conditions hold for a prompt and the events of its turn (origin and surface
    rules share them)."""
    if when.text_prefix is not None and not typed.startswith(when.text_prefix):
        return False
    if when.text_regex is not None and not when.text_regex.match(typed):
        return False
    if when.field is not None and (prompt.get("fields") or {}).get(when.field) in NO_VALUE:
        return False
    if when.event is None:
        return _value_matches(prompt.get("raw") or {}, when)
    return any(e["event"] == when.event and _value_matches(e["raw"], when) for e in events)


def origin_of(prompt: dict[str, Any], events: list[dict[str, Any]], rules: list[OriginRule], typed: str) -> Origin:
    return next((r.origin for r in rules if matches(r.when, prompt, events, typed)), DEFAULT)


def classify(prompt: dict[str, Any], events: list[dict[str, Any]], spec: Spec | None) -> dict[str, Any]:
    """A stored prompt row (with `prompt`, `fields`, `raw`) and the events of its turn -> its origin,
    typed text and injected context. `unknown` when there is no text, nothing was typed, or the
    prompt's source has no spec any more."""
    text = prompt.get("prompt")
    if not isinstance(text, str):
        return {"origin": "unknown", "typed_text": None, "injected": []}
    if spec is None:
        return {"origin": "unknown", "typed_text": text, "injected": []}
    typed, injected = split_context(text, spec.injected)
    origin = origin_of(prompt, events, spec.origin, typed) if typed else "unknown"
    return {"origin": origin, "typed_text": typed, "injected": injected}
