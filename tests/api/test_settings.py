"""Settings page API: preferences, data clean-ups (dry run, then apply) and token rotation."""

import json
from datetime import UTC, datetime, timedelta

import pytest

from gitprompt.capture import retention
from gitprompt.core.preferences import PreferenceFile
from gitprompt.storage.store import Store

DASHBOARD = {"X-GitPrompt": "1"}


def auth(settings):
    return {"Authorization": f"Bearer {settings.token()}"}


# --- preferences ---


def test_settings_report_value_and_source(client, settings, monkeypatch):
    monkeypatch.setenv("GITPROMPT_WEEK_START", "sunday")
    body = client.get("/api/settings").json()
    assert body["file"] == str(settings.home / "preferences.json")
    assert body["settings"]["default_range"] == {
        "value": "7d",
        "default": "7d",
        "source": "default",
        "env": "GITPROMPT_DEFAULT_RANGE",
        "locked": False,
        "applies": "now",
    }
    assert body["settings"]["week_start"]["source"] == "env" and body["settings"]["week_start"]["locked"]


def test_put_settings(client, settings, monkeypatch):
    assert client.put("/api/settings", json={"default_range": "30d"}).status_code == 403  # needs the dashboard header
    saved = client.put("/api/settings", json={"default_range": "30d"}, headers=DASHBOARD).json()
    assert saved["changed"] == ["default_range"] and not saved["restart_required"]
    assert saved["settings"]["default_range"]["source"] == "file"
    assert json.loads((settings.home / "preferences.json").read_text()) == {"default_range": "30d"}

    assert client.put("/api/settings", json={"default_range": "1y"}, headers=DASHBOARD).status_code == 422
    monkeypatch.setenv("GITPROMPT_DEFAULT_RANGE", "24h")
    locked = client.put("/api/settings", json={"default_range": "all"}, headers=DASHBOARD)
    assert locked.status_code == 409 and "GITPROMPT_DEFAULT_RANGE" in locked.text


def test_restart_is_reported_for_start_up_settings(client, monkeypatch):
    monkeypatch.setattr("gitprompt.api.routes.settings.APPLIES_AT_START", frozenset({"week_start"}))
    saved = client.put("/api/settings", json={"week_start": "sunday"}, headers=DASHBOARD).json()
    assert saved["restart_required"]


def test_hide_secrets_on_ingest(client, settings, hook_payload):
    secret = "AKIA" + "ABCDEFGHIJKLMNOP"
    client.post("/ingest/claude-code", json=hook_payload | {"prompt": f"use {secret}"}, headers=auth(settings))
    client.put("/api/settings", json={"hide_secrets_on_ingest": True}, headers=DASHBOARD)  # applies at once
    later = hook_payload | {"prompt_id": "p2", "prompt": f"again {secret}"}
    client.post("/ingest/claude-code", json=later, headers=auth(settings))
    newest, oldest = client.get("/api/prompts").json()
    assert newest["prompt"] == "again [hidden AWS access key]"
    assert secret not in json.dumps(client.get(f"/api/prompts/{newest['id']}").json())  # raw payload too
    assert secret in oldest["prompt"]  # stored before the change: hide it from the prompt page


# --- data ---


@pytest.fixture
def events(settings):
    """Three stored events: one noise kind the spec drops, one old and one recent api_request."""
    store = Store(settings.db_path)
    rows = [("claude-code-otel", "hook_registered", 0), ("claude-code-otel", "api_request", 40), ("claude-code", "Stop", 0)]
    for source, event, days in rows:
        row = store.add_event(source, {"event": event, "session_id": "s1"}, {"x": event})
        received = (datetime.now(UTC) - timedelta(days=days)).isoformat()
        store.db.execute("update event set received_at = ? where id = ?", (received, row["id"]))
    yield store
    store.close()


def count(store: Store) -> int:
    return store.db.execute("select count(*) from event").fetchone()[0]


def test_data_overview(client, settings, events):
    settings.backups.mkdir(parents=True)
    (settings.backups / "settings.json.20261107-101010").write_text("{}")
    body = client.get("/api/data").json()
    assert body["db_path"] == str(settings.db_path) and body["db_bytes"] > 0
    assert {s["source"]: s["rows"] for s in body["events"]} == {"claude-code-otel": 2, "claude-code": 1}
    assert [b["name"] for b in body["backups"]] == ["settings.json.20261107-101010"]


def test_compact_previews_then_applies(client, events):
    preview = client.post("/api/data/compact", json={"older_than_days": 30}, headers=DASHBOARD).json()
    assert not preview["applied"] and preview["rows"] == 2 and count(events) == 3  # dry run by default
    why = {(r["event"], r["why"]) for r in preview["removals"]}
    assert why == {("hook_registered", "dropped by spec"), ("api_request", "expired")}
    applied = client.post("/api/data/compact?dry_run=false", json={"older_than_days": 30}, headers=DASHBOARD).json()
    assert applied["applied"] and applied["size_after"] is not None and count(events) == 1


def test_delete_events_by_source_and_date(client, events):
    assert client.post("/api/data/delete-events", json={}, headers=DASHBOARD).status_code == 400  # never "everything"
    cutoff = (datetime.now(UTC) - timedelta(days=10)).isoformat()
    preview = client.post("/api/data/delete-events", json={"before": cutoff}, headers=DASHBOARD).json()
    assert [(r["source"], r["event"]) for r in preview["removals"]] == [("claude-code-otel", "api_request")]
    by_source = client.post("/api/data/delete-events", json={"source": "claude-code"}, headers=DASHBOARD).json()
    assert by_source["rows"] == 1 and count(events) == 3
    before = by_source["removals"][0]["before"]  # the dry run fixed "until now": confirm with the same time
    body = {"source": "claude-code", "before": before}
    assert client.post("/api/data/delete-events?dry_run=false", json=body, headers=DASHBOARD).json()["applied"]
    assert count(events) == 2


def test_retention_pass_follows_the_preference(settings, events):
    prefs = PreferenceFile(settings.home, environ={})
    assert retention.apply(events, prefs) == 0 and count(events) == 3  # off by default
    prefs.update({"event_retention_days": 30})
    assert retention.apply(events, prefs) == 1 and count(events) == 2


# --- security ---


def test_rotate_token_reconnects_connected_sources(client, settings, claude_settings):
    claude_settings.write_text("{}")
    client.post("/api/sources/claude-code/connect", headers=DASHBOARD)
    old = settings.token()
    assert client.get("/api/security").json()["connected"] == ["Claude Code"]

    preview = client.post("/api/security/rotate-token", headers=DASHBOARD).json()
    [reconnect] = preview["reconnects"]
    assert not preview["applied"] and reconnect["source"] == "claude-code"
    assert "<new token>" in reconnect["change"]["diff"]
    assert settings.token() == old and old in claude_settings.read_text()  # the dry run changes nothing

    applied = client.post("/api/security/rotate-token?dry_run=false", headers=DASHBOARD).json()
    new = settings.token()
    assert applied["applied"] and new != old and settings.token_file.stat().st_mode & 0o777 == 0o600
    written = claude_settings.read_text()
    assert new in written and old not in written and applied["reconnects"][0]["change"]["backup"]
    stale = {"Authorization": f"Bearer {old}"}
    assert client.post("/ingest/claude-code", json={"hook_event_name": "Stop"}, headers=stale).status_code == 401


def test_rotation_is_refused_when_a_source_cannot_be_reconnected(client, settings, claude_settings):
    claude_settings.write_text("{}")
    client.post("/api/sources/claude-code/connect", headers=DASHBOARD)
    doc = json.loads(claude_settings.read_text()) | {"disableAllHooks": True}
    claude_settings.write_text(json.dumps(doc))
    old = settings.token()
    preview = client.post("/api/security/rotate-token", headers=DASHBOARD).json()
    assert preview["blocked"] and not preview["reconnects"]
    assert client.post("/api/security/rotate-token?dry_run=false", headers=DASHBOARD).status_code == 409
    assert settings.token() == old
