"""A second tool end to end (GitHub Copilot CLI, data only), and connector management: capabilities,
the spec in use, enable/disable. Synthetic payloads in the documented PascalCase form."""

import json

import pytest

DASHBOARD = {"X-GitPrompt": "1"}
SESSION = "8f14e45f-ceea-467f-a0e6-2c1d1a9b7e10"


def auth(settings):
    return {"Authorization": f"Bearer {settings.token()}"}


def source(client, id):
    return next(s for s in client.get("/api/sources").json() if s["id"] == id)


def copilot(event, **extra):
    return {"hook_event_name": event, "session_id": SESSION, "timestamp": "2026-11-07T10:00:00.000Z", "cwd": "/work/app", **extra}


@pytest.fixture
def hooks_dir(settings):
    path = settings.user_home / ".copilot"
    path.mkdir(parents=True)
    return path / "hooks"


def test_copilot_connect_writes_a_hooks_file_of_its_own_and_disconnect_empties_it(client, hooks_dir):
    assert source(client, "copilot-cli")["detected"]
    applied = client.post("/api/sources/copilot-cli/connect", headers=DASHBOARD).json()
    written = json.loads((hooks_dir / "gitprompt.json").read_text())
    assert applied["applied"] and written["version"] == 1
    [handler] = written["hooks"]["UserPromptSubmit"]
    assert handler["type"] == "command" and "/ingest/copilot-cli" in handler["bash"] and "{{" not in handler["bash"]
    assert source(client, "copilot-cli")["connected"] and not source(client, "claude-code")["connected"]

    client.post("/api/sources/copilot-cli/disconnect", headers=DASHBOARD)
    assert json.loads((hooks_dir / "gitprompt.json").read_text()) == {}
    assert not source(client, "copilot-cli")["connected"]


def test_copilot_policy_file_blocks_connecting(client, hooks_dir, settings, monkeypatch):
    settings_file = hooks_dir.parent / "settings.json"
    settings_file.write_text(json.dumps({"disableAllHooks": True}))
    assert source(client, "copilot-cli")["blocks"]
    assert client.post("/api/sources/copilot-cli/connect", headers=DASHBOARD).status_code == 409


def test_copilot_events_join_their_prompt_by_session_order(client, settings):
    send = lambda payload: client.post("/ingest/copilot-cli", json=payload, headers=auth(settings))  # noqa: E731
    send(copilot("UserPromptSubmit", prompt="Add a retry to the upload client"))
    send(copilot("PostToolUse", tool_name="bash", tool_input={"command": "pytest"}, tool_result={"result_type": "success"}))
    send(copilot("PostToolUseFailure", tool_name="edit", tool_input={"path": "a.py"}, error="File not found"))
    send(copilot("Stop", stop_reason="end_turn", stop_hook_active=False))
    send(copilot("UserPromptSubmit", prompt="/review the staged changes"))
    send(copilot("PostToolUse", tool_name="view", tool_input={"path": "a.py"}))

    review, first = client.get("/api/prompts?origin=typed&origin=slash_command").json()
    assert first["fields"]["prompt_id"] == f"copilot-cli#{first['id']}" and first["origin"] == "typed"
    assert first["summary"]["tool_calls"] == {"bash": 1, "edit": 1} and first["summary"]["tool_failures"] == 1
    assert first["summary"]["ended_by"] == "Stop"
    assert review["origin"] == "slash_command" and review["summary"]["tool_calls"] == {"view": 1}

    # Re-mapping rebuilds fields from raw, which holds no prompt id: the join is redone, not lost.
    client.post("/api/sources/copilot-cli/remap", headers=DASHBOARD)
    again = {p["id"]: p for p in client.get("/api/prompts?origin=typed&origin=slash_command").json()}
    assert again[first["id"]]["fields"]["prompt_id"] == first["fields"]["prompt_id"]
    assert again[first["id"]]["summary"]["tool_calls"] == {"bash": 1, "edit": 1}
    assert again[review["id"]]["summary"]["tool_calls"] == {"view": 1}


def test_sources_report_capabilities_join_and_spec_file(client, settings):
    hooks, telemetry, copilot_cli = (source(client, i) for i in ("claude-code", "claude-code-otel", "copilot-cli"))
    assert "usage" in telemetry["capabilities"] and "usage" not in hooks["capabilities"]
    assert "model" not in copilot_cli["capabilities"] and "turn_end" in copilot_cli["capabilities"]
    assert copilot_cli["join"] == "session" and hooks["join"] == "prompt_id"
    assert copilot_cli["spec"]["origin"] == "bundled" and copilot_cli["spec"]["name"] == "copilot-cli.yaml"
    assert copilot_cli["spec_problem"] is None

    text = client.get("/api/sources/copilot-cli/spec").json()
    assert text["file"]["path"].endswith("copilot-cli.yaml") and "id: copilot-cli" in text["text"]


def test_a_broken_override_is_shown_on_the_bundled_connector(client, settings):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "copilot-cli.yaml").write_text("id: copilot-cli\n")
    found = source(client, "copilot-cli")
    assert found["spec"]["origin"] == "bundled" and found["spec_problem"]["file"]["overrides"]
    assert "copilot-cli.yaml" in found["spec_problem"]["error"]
    [problem] = client.get("/api/sources/problems").json()
    assert problem["fallback"] == "copilot-cli"


def test_a_disabled_connector_discards_payloads_and_refuses_to_connect(client, settings, hook_payload, hooks_dir):
    assert client.post("/api/sources/copilot-cli/disable").status_code == 403  # writes need the header
    assert client.post("/api/sources/copilot-cli/disable", headers=DASHBOARD).json() == {"enabled": False}
    assert not source(client, "copilot-cli")["enabled"] and source(client, "claude-code")["enabled"]

    response = client.post("/ingest/copilot-cli", json=copilot("UserPromptSubmit", prompt="hi"), headers=auth(settings))
    assert response.status_code == 204 and source(client, "copilot-cli")["total"] == 0
    client.post("/ingest/claude-code", json=hook_payload, headers=auth(settings))
    assert source(client, "claude-code")["total"] == 1  # other tools are untouched
    assert client.post("/api/sources/copilot-cli/connect?dry_run=true", headers=DASHBOARD).status_code == 409

    assert client.post("/api/sources/copilot-cli/enable", headers=DASHBOARD).json() == {"enabled": True}
    client.post("/ingest/copilot-cli", json=copilot("UserPromptSubmit", prompt="hi"), headers=auth(settings))
    assert source(client, "copilot-cli")["total"] == 1
    assert client.get("/api/settings").json()["settings"]["disabled_sources"]["value"] == []


def test_disabled_sources_can_be_set_by_env(client, monkeypatch):
    monkeypatch.setenv("GITPROMPT_DISABLED_SOURCES", "copilot-cli, claude-code-otel")
    assert not source(client, "copilot-cli")["enabled"] and not source(client, "claude-code-otel")["enabled"]
    assert client.post("/api/sources/copilot-cli/enable", headers=DASHBOARD).status_code == 409  # locked by env
