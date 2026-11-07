"""The places a contributor extends GitPrompt: derivation registries, capture decoders, policy rules,
spec overrides. Each test pins the contract that makes the extension work without other edits."""

import asyncio
import json

import pytest

from gitprompt.api.bus import Bus
from gitprompt.capture import pipeline
from gitprompt.core.errors import SpecError
from gitprompt.insights import derivations
from gitprompt.sources import probe
from gitprompt.sources.spec import load_specs
from gitprompt.storage.store import Store

DASHBOARD = {"X-GitPrompt": "1"}


def test_a_registered_derivation_is_computed_and_documented():
    registry: derivations.Derivations[str] = derivations.Derivations("stats")

    @registry({"shouting": "prompt is all caps"})
    def _shouting(text: str) -> dict:
        return {"shouting": text.isupper()}

    assert registry.derive("HELLO") == {"shouting": True}
    assert registry.derive("hello") == {}  # empty and false values are left out
    assert registry.docs == {"stats.shouting": "prompt is all caps"}


def test_every_turn_value_is_documented_on_the_fields_page():
    events = [
        {"event": "PostToolUse", "received_at": "2026-11-03T10:00:01+00:00", "raw": {"tool_name": "mcp__x__y", "duration_ms": 900}},
        {"event": "Stop", "received_at": "2026-11-03T10:00:02+00:00", "raw": {"last_assistant_message": "ok"}},
    ]
    summary = derivations.turn({"received_at": "2026-11-03T10:00:00+00:00", "turn": 1}, events)
    documented = " / ".join(k for k in derivations.DERIVED if k.startswith("summary."))
    assert all(key in documented for key in summary), summary.keys()


def test_each_spec_format_has_a_decoder(settings):
    assert {spec.format for spec in load_specs(settings).values()} <= pipeline.DECODERS.keys()


def test_otlp_endpoint_refuses_a_hooks_source(client, settings):
    response = client.post("/ingest/claude-code/v1/logs", json={}, headers={"Authorization": f"Bearer {settings.token()}"})
    assert response.status_code == 404


def test_policy_rules_report_blocks_and_warnings(settings, claude_settings, monkeypatch):
    specs = load_specs(settings)
    claude_settings.write_text(json.dumps({"allowedHttpHookUrls": ["https://x"], "env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://c"}}))
    monkeypatch.setenv("OTEL_LOGS_EXPORTER", "otlp")

    hooks = probe.check_policy(specs["claude-code"], settings)
    assert hooks.blocks == [] and hooks.warnings == []  # the URL allowlist only governs HTTP hooks; ours are command hooks

    otel = probe.check_policy(specs["claude-code-otel"], settings)
    assert "OTEL_EXPORTER_OTLP_ENDPOINT" in otel.blocks[0] and any("OTEL_LOGS_EXPORTER" in w for w in otel.warnings)


def test_a_broken_user_spec_names_the_file(client, settings):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "mytool.yaml").write_text("id: mytool\n")  # missing required keys
    with pytest.raises(SpecError, match="mytool.yaml"):
        load_specs(settings)
    response = client.get("/api/sources")
    assert response.status_code == 500 and "mytool.yaml" in response.json()["detail"]


def test_store_rejects_unknown_filters(tmp_path):
    with pytest.raises(ValueError, match="nope"):
        Store(tmp_path / "db").search(nope="x")


def test_writes_to_a_missing_prompt_are_404(client):
    assert client.put("/api/prompts/999/annotation", json={}, headers=DASHBOARD).status_code == 404


def test_bus_drops_notices_for_a_stalled_subscriber():
    async def scenario():
        bus = Bus(backlog=1)
        async with bus.subscribe() as queue:
            bus.publish({"n": 1})
            bus.publish({"n": 2})  # queue full: dropped, not raised
            assert queue.qsize() == 1
        bus.publish({"n": 3})  # no subscribers left

    asyncio.run(scenario())
