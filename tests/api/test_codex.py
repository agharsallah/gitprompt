"""OpenAI Codex CLI end to end through the API, data only (codex.yaml): connecting merges into the
user's own $CODEX_HOME/hooks.json and disconnecting restores it, TOML settings that turn hooks off
block connecting, and a turn's events join their prompt by `turn_id`. Synthetic payloads in the
shape of the 0.150.1 hook input schemas."""

import json

import pytest

DASHBOARD = {"X-GitPrompt": "1"}
SESSION = "0190a1b2-0000-7000-8000-000000000001"
TURN = "0190a1b2-0000-7000-8000-0000000000a1"
USER_HOOKS = {"description": "mine", "hooks": {"PreToolUse": [{"matcher": "^Bash$", "hooks": [{"type": "command", "command": "true"}]}]}}


def source(client, id):
    return next(s for s in client.get("/api/sources").json() if s["id"] == id)


def codex(event, **extra):
    common = {"session_id": SESSION, "transcript_path": "/t.jsonl", "cwd": "/work/app", "model": "gpt-5-codex"}
    return {**common, "hook_event_name": event, **extra}


@pytest.fixture
def codex_home(tmp_path, monkeypatch):
    home = tmp_path / "codex-home"
    home.mkdir()
    monkeypatch.setenv("CODEX_HOME", str(home))
    return home


def test_connect_merges_into_the_users_hooks_file_and_disconnect_restores_it(client, codex_home):
    hooks = codex_home / "hooks.json"
    before = json.dumps(USER_HOOKS, indent=2) + "\n"
    hooks.write_text(before)
    found = source(client, "codex")
    assert found["detected"] and found["config_file"] == str(hooks) and found["assistant"] == "Codex"
    assert "/hooks" in found["after_connect"]

    assert client.post("/api/sources/codex/connect", headers=DASHBOARD).json()["applied"]
    written = json.loads(hooks.read_text())
    assert written["description"] == "mine" and written["hooks"]["PreToolUse"] == USER_HOOKS["hooks"]["PreToolUse"]
    [prompt_hook] = written["hooks"]["UserPromptSubmit"][0]["hooks"]
    [end_hook] = written["hooks"]["SessionEnd"][0]["hooks"]
    assert prompt_hook["async"] is True and "/ingest/codex" in prompt_hook["command"] and "{{" not in prompt_hook["command"]
    assert "async" not in end_hook and end_hook["timeout"] <= 3  # Codex runs SessionEnd synchronously, 3 s at most
    assert "async" not in written["hooks"]["Stop"][0]["hooks"][0]  # an async Stop is cancelled when `codex exec` ends
    assert source(client, "codex")["connected"]

    client.post("/api/sources/codex/disconnect", headers=DASHBOARD)
    assert hooks.read_text() == before and not source(client, "codex")["connected"]


@pytest.mark.parametrize(
    ("file", "text"),
    [
        ("config.toml", "[features]\nhooks = false\n"),
        ("config.toml", "[features]\ncodex_hooks = false\n"),
    ],
)
def test_toml_settings_that_turn_hooks_off_block_connecting(client, codex_home, file, text):
    (codex_home / file).write_text(text)
    assert source(client, "codex")["blocks"]
    assert client.post("/api/sources/codex/connect", headers=DASHBOARD).status_code == 409


def test_unrelated_or_broken_toml_does_not_block(client, codex_home):
    (codex_home / "config.toml").write_text('model = "gpt-5-codex"\n[features]\nhooks = true\n')
    assert source(client, "codex")["blocks"] == []
    (codex_home / "config.toml").write_text("[features\nhooks = false\n")  # Codex can't apply it either
    assert source(client, "codex")["blocks"] == []


def test_a_turn_joins_its_prompt_by_turn_id_and_counts_its_model(client, settings, codex_home):
    auth = {"Authorization": f"Bearer {settings.token()}"}
    send = lambda payload: client.post("/ingest/codex", json=payload, headers=auth)  # noqa: E731
    patch = "*** Begin Patch\n*** Update File: /work/app/upload.py\n@@\n-a\n+b\n*** End Patch\n"
    # Codex runs these hooks async: a tool result can arrive before its prompt.
    send(codex("PostToolUse", turn_id=TURN, tool_name="apply_patch", tool_use_id="c1", tool_input={"command": patch}, tool_response="ok"))
    send(codex("UserPromptSubmit", turn_id=TURN, permission_mode="default", prompt="Add a retry to the upload client"))
    send(codex("PostToolUse", turn_id=TURN, tool_name="Bash", tool_use_id="c2", tool_input={"command": "pytest -q"}, tool_response="ok"))
    send(codex("Stop", turn_id=TURN, stop_hook_active=False, last_assistant_message="Added a retry."))

    [prompt] = client.get("/api/prompts?source=codex").json()
    assert prompt["fields"]["prompt_id"] == TURN and prompt["origin"] == "typed"
    summary = prompt["summary"]
    assert summary["tool_kinds"] == {"write": 1, "shell": 1} and summary["files_written"] == ["/work/app/upload.py"]
    assert summary["response"] == "Added a retry." and summary["ended_by"] == "Stop"

    [model] = client.get("/api/stats?source=codex&group_by=model").json()["groups"]
    assert (model["key"], model["prompts"], model["api_requests"]) == ("gpt-5-codex", 1, 0)  # from the hooks, no usage
    story = client.get(f"/api/stories/session/{SESSION}").json()
    assert story["items"][0]["files"] == ["/work/app/upload.py"]
    overview = client.get("/api/export/overview?source=codex&format=md").text
    assert "**Prompts:** 1" in overview and "gpt-5-codex" in overview
    assert "**Cost:**" not in overview and "**Tokens:**" not in overview  # no tool reported usage: left out, not $0
