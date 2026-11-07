"""The capture pipeline, shared by every channel:

    request body --decode (spec.format)--> payloads --map (spec.map)--> prompt or event --> store

Events the spec does not keep (`accept_events`, `drop_events`) are discarded before storage.
`Captured` says which turns got events, so the caller can mark them for the prompt index.
A payload whose event is one of `spec.prompt_events` becomes a prompt, stamped with what GitPrompt
observes on this machine at that moment (git state, tool version). Every other payload is stored
as an event and attaches to its prompt by prompt_id / session_id.

To support a new wire format, add a decoder to DECODERS and its name to sources/spec.py `Format`.
"""

import asyncio
from collections.abc import Callable, Iterable
from dataclasses import dataclass, field
from typing import Any

from gitprompt.capture import otlp
from gitprompt.core.git import git_context
from gitprompt.core.payload import clip, extract
from gitprompt.insights.redaction import redact
from gitprompt.sources import probe
from gitprompt.sources.spec import Spec
from gitprompt.storage.store import Store

Decoder = Callable[[dict[str, Any]], Iterable[dict[str, Any]]]

DECODERS: dict[str, Decoder] = {
    "json": lambda body: [body],  # one event per request (hooks)
    "otlp": otlp.records,  # OTLP/HTTP JSON: a batch of log records and metric points
}


@dataclass
class Captured:
    prompts: int = 0
    events: int = 0
    turns: set[str] = field(default_factory=set)  # prompt_ids that received events

    def __bool__(self) -> bool:
        return bool(self.prompts or self.events)

    def notice(self) -> dict[str, int]:
        return {"prompts": self.prompts, "events": self.events}


def observe(spec: Spec, cwd: str | None) -> dict[str, Any]:
    """What GitPrompt sees on this machine when a prompt arrives: repository state, tool version."""
    version = probe.tool_version(spec)
    return git_context(cwd) | ({"tool_version": version} if version else {})


async def capture(spec: Spec, store: Store, body: dict[str, Any], hide_secrets: bool = False) -> Captured:
    """`hide_secrets` (preference `hide_secrets_on_ingest`): replace detected secrets before storing."""
    result = Captured()
    for payload in DECODERS[spec.format](body):
        if hide_secrets:
            payload = redact(payload)
        fields = extract(payload, spec.map)
        event = fields.get("event")
        if not spec.keeps(event):  # accept_events / drop_events in the spec
            continue
        if event in spec.prompt_events:
            context = await asyncio.to_thread(observe, spec, fields.get("cwd"))
            result.prompts += store.add_prompt(spec.id, fields, context, payload) is not None  # None = duplicate
        else:
            store.add_event(spec.id, fields, clip(payload))
            result.events += 1
            if fields.get("prompt_id"):
                result.turns.add(fields["prompt_id"])
    return result
