import json

DASHBOARD = {"X-GitPrompt": "1"}


def auth(settings):
    return {"Authorization": f"Bearer {settings.token()}"}


def source(client, id="claude-code"):
    return next(s for s in client.get("/api/sources").json() if s["id"] == id)


def test_connect_ingest_disconnect(client, settings, claude_settings, hook_payload):
    original = {"theme": "dark", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}}
    claude_settings.write_text(json.dumps(original))
    claude_settings.chmod(0o600)

    found = source(client)
    assert found["detected"] and not found["connected"]

    # Writes need the dashboard header (cross-site requests can't send it).
    assert client.post("/api/sources/claude-code/connect").status_code == 403

    preview = client.post("/api/sources/claude-code/connect?dry_run=true", headers=DASHBOARD).json()
    assert "/ingest/claude-code" in preview["diff"] and not preview["applied"]
    assert json.loads(claude_settings.read_text()) == original

    applied = client.post("/api/sources/claude-code/connect", headers=DASHBOARD).json()
    assert applied["applied"] and applied["backup"]
    written = json.loads(claude_settings.read_text())
    assert written["theme"] == "dark" and "Stop" in written["hooks"]
    assert claude_settings.stat().st_mode & 0o777 == 0o600
    assert source(client)["connected"] and not source(client, "claude-code-otel")["connected"]

    # The hook call: needs the token, and always gets an empty 2xx (Claude Code adds nothing to the prompt).
    assert client.post("/ingest/claude-code", json=hook_payload).status_code == 401
    response = client.post("/ingest/claude-code", json=hook_payload, headers=auth(settings))
    assert response.status_code == 204 and response.content == b""
    client.post("/ingest/claude-code", json=hook_payload, headers=auth(settings))  # same prompt_id
    client.post("/ingest/claude-code", json=hook_payload | {"hook_event_name": "Stop"}, headers=auth(settings))

    [prompt] = client.get("/api/prompts").json()
    assert prompt["prompt"] == hook_payload["prompt"]
    assert prompt["fields"]["session_id"] == "abc123" and prompt["fields"]["cwd"] == hook_payload["cwd"]
    assert client.get(f"/api/prompts/{prompt['id']}").json()["raw"] == hook_payload
    assert client.get("/api/prompts?q=factorial").json() and not client.get("/api/prompts?q=nothing").json()

    removed = client.post("/api/sources/claude-code/disconnect", headers=DASHBOARD).json()
    assert removed["applied"]
    assert json.loads(claude_settings.read_text()) == original


def test_remap_applies_an_edited_spec(client, settings, hook_payload):
    client.post("/ingest/claude-code", json=hook_payload | {"prompt": None, "text": "typed"}, headers=auth(settings))
    assert source(client)["unmapped"] == 0  # "text" is a default candidate
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "claude-code.yaml").write_text(
        "id: claude-code\nname: Claude Code\ndetect: []\ninstall: {file: x, merge: {}}\nmap: {prompt: [nope]}\n"
    )
    assert client.post("/api/sources/claude-code/remap", headers=DASHBOARD).json() == {"updated": 1}
    assert source(client)["unmapped"] == 1


def test_policy_blocks_install(client, claude_settings):
    claude_settings.write_text(json.dumps({"disableAllHooks": True}))
    response = client.post("/api/sources/claude-code/connect", headers=DASHBOARD)
    assert response.status_code == 409 and "disableAllHooks" in response.text
    assert json.loads(claude_settings.read_text()) == {"disableAllHooks": True}


def test_rejects_unknown_hosts(client):
    assert client.get("/api/sources", headers={"Host": "evil.example"}).status_code == 400


def test_refuses_to_touch_invalid_settings(client, claude_settings):
    claude_settings.write_text("{oops")
    assert client.post("/api/sources/claude-code/connect", headers=DASHBOARD).status_code == 409
    assert claude_settings.read_text() == "{oops"


def otlp(prompt_id, session_id="abc123"):
    """OTLP/HTTP JSON as Claude Code exports it (attributes per the monitoring docs, synthetic values)."""

    def attrs(**kv):
        return [{"key": k.replace("_", ".", 1) if k in ("prompt_id", "session_id", "event_name") else k, "value": v} for k, v in kv.items()]

    return {
        "resourceLogs": [
            {
                "resource": {"attributes": attrs(**{"service.name": {"stringValue": "claude-code"}})},
                "scopeLogs": [
                    {
                        "logRecords": [
                            {
                                "body": {"stringValue": "claude_code.api_request"},
                                "attributes": attrs(
                                    event_name={"stringValue": "api_request"},
                                    prompt_id={"stringValue": prompt_id},
                                    session_id={"stringValue": session_id},
                                    model={"stringValue": "claude-haiku"},
                                    input_tokens={"intValue": "120"},
                                    cost_usd={"doubleValue": 0.002},
                                ),
                            }
                        ]
                    }
                ],
            }
        ]
    }


def test_events_and_telemetry_enrich_a_prompt(client, settings, hook_payload):
    pid = hook_payload["prompt_id"]
    client.post("/ingest/claude-code", json=hook_payload | {"prompt": "ship it, password=hunter2hunter2"}, headers=auth(settings))
    tool = {"tool_name": "Write", "tool_input": {"file_path": "/r/a.py", "content": "x" * 20_000}, "duration_ms": 40}
    client.post("/ingest/claude-code", json=hook_payload | {"hook_event_name": "PostToolUse"} | tool, headers=auth(settings))
    client.post(
        "/ingest/claude-code", json=hook_payload | {"hook_event_name": "Stop", "last_assistant_message": "Done"}, headers=auth(settings)
    )
    response = client.post("/ingest/claude-code-otel/v1/logs", json=otlp(pid), headers=auth(settings))
    assert response.status_code == 200 and response.json() == {}

    [listed] = client.get("/api/prompts").json()
    assert listed["summary"]["tool_calls"] == {"Write": 1} and listed["summary"]["input_tokens"] == 120
    assert listed["stats"]["secrets"] == ["password or key assignment"] and listed["context"]["tool_version"] == "2.1.316"

    detail = client.get(f"/api/prompts/{listed['id']}").json()
    assert detail["summary"]["response"] == "Done" and detail["summary"]["files_written"] == ["/r/a.py"]
    assert [e["event"] for e in detail["events"]] == ["PostToolUse", "Stop", "api_request"]
    assert len(detail["events"][0]["raw"]["tool_input"]["content"]) < 10_100  # long values are clipped
    assert detail["session"]["environment"] == {"service.name": "claude-code"}

    # The user's own notes, and hiding the detected secret everywhere it was stored.
    note = {"starred": True, "rating": 1, "tags": ["release"], "note": "good one"}
    assert client.put(f"/api/prompts/{listed['id']}/annotation", json=note).status_code == 403
    assert client.put(f"/api/prompts/{listed['id']}/annotation", json=note, headers=DASHBOARD).json() == note
    assert client.get("/api/prompts?starred=true&tag=release").json()[0]["annotation"] == note
    assert not client.get("/api/prompts?tag=other").json()
    assert client.post(f"/api/prompts/{listed['id']}/redact", headers=DASHBOARD).json() == {"updated": 4}
    assert "hunter2" not in client.get(f"/api/prompts/{listed['id']}").text

    fields = client.get("/api/fields").json()
    seen = {(f["source"], f["event"], f["path"]): f for f in fields["fields"]}
    assert seen[("claude-code", "PostToolUse", "tool_input.file_path")]["documented"]
    assert seen[("claude-code-otel", "api_request", "input_tokens")]["count"] == 1
    assert seen[("claude-code", "SessionEnd", "reason")]["count"] == 0  # documented, not seen yet
    assert any(n["event"] == "SessionStart" for n in fields["not_captured"]) and fields["derived"]


def test_telemetry_connect_keeps_existing_env_and_disconnect_restores(client, claude_settings):
    original = {"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1", "MY_VAR": "x"}}
    claude_settings.write_text(json.dumps(original))
    client.post("/api/sources/claude-code/connect", headers=DASHBOARD)
    client.post("/api/sources/claude-code-otel/connect", headers=DASHBOARD)
    written = json.loads(claude_settings.read_text())
    assert written["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"].endswith("/ingest/claude-code-otel") and "Stop" in written["hooks"]
    assert source(client, "claude-code-otel")["connected"] and not source(client, "claude-code-otel")["blocks"]
    assert client.post("/api/sources/claude-code-otel/connect?dry_run=true", headers=DASHBOARD).json()["diff"] == ""

    client.post("/api/sources/claude-code-otel/disconnect", headers=DASHBOARD)
    assert json.loads(claude_settings.read_text()) == original | {"hooks": written["hooks"]}  # hooks source untouched
    client.post("/api/sources/claude-code/disconnect", headers=DASHBOARD)
    assert json.loads(claude_settings.read_text()) == original


def test_telemetry_is_not_redirected_when_already_configured(client, claude_settings):
    claude_settings.write_text(json.dumps({"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "https://collector.example.com"}}))
    response = client.post("/api/sources/claude-code-otel/connect", headers=DASHBOARD)
    assert response.status_code == 409 and "OTEL_EXPORTER_OTLP_ENDPOINT" in response.text


def test_policy_lock_from_strict_plugin_only(client, claude_settings):
    claude_settings.write_text(json.dumps({"strictPluginOnlyCustomization": ["skills", "hooks"]}))
    assert client.post("/api/sources/claude-code/connect", headers=DASHBOARD).status_code == 409


def test_older_connection_is_reported_and_updated_in_place(client, claude_settings):
    old = {"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:7878/ingest/claude-code"}]}]}}
    claude_settings.write_text(json.dumps(old))
    assert source(client)["connected"] and source(client)["outdated"]
    client.post("/api/sources/claude-code/connect", headers=DASHBOARD)
    written = json.loads(claude_settings.read_text())
    assert len(written["hooks"]["UserPromptSubmit"]) == 1 and "Stop" in written["hooks"]  # replaced, not duplicated
    assert not source(client)["outdated"]
