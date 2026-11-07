import pytest

from gitprompt.sources import installer
from gitprompt.sources.spec import marker

OURS = {"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:7878/ingest/claude-code"}]}]}}
OURS_RE = marker("claude-code")
USER = {
    "theme": "dark",
    "hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": "./mine.sh"}]}], "Stop": []},
}


def test_merge_then_strip_restores_the_original():
    merged = installer.merge(USER, OURS)
    assert len(merged["hooks"]["UserPromptSubmit"]) == 2
    assert installer.strip(merged, OURS_RE) == USER


def test_strip_removes_containers_it_emptied_only():
    assert installer.strip(installer.merge({"x": 1}, OURS), OURS_RE) == {"x": 1}


def test_merge_is_idempotent():
    once = installer.merge(USER, OURS)
    assert installer.merge(once, OURS) == once


def test_read_refuses_invalid_json(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text("{ // comment\n}")
    with pytest.raises(installer.ConfigError):
        installer.read(path)


def test_marker_does_not_match_a_longer_source_id():
    assert marker("claude-code").search("http://x/ingest/claude-code")
    assert not marker("claude-code").search("http://x/ingest/claude-code-otel")


def test_uninstall_removes_only_values_we_added_and_left_unchanged():
    user = {"env": {"KEEP": "1", "CLAUDE_CODE_ENABLE_TELEMETRY": "1"}}
    patch = {"env": {"CLAUDE_CODE_ENABLE_TELEMETRY": "1", "OTEL_LOGS_EXPORTER": "otlp", "OTEL_PROTOCOL": "http/json"}}
    record = installer.added(user, patch)
    assert record == [(["env", "OTEL_LOGS_EXPORTER"], "otlp"), (["env", "OTEL_PROTOCOL"], "http/json")]
    merged = installer.merge(user, patch)
    merged["env"]["OTEL_PROTOCOL"] = "grpc"  # edited by the user after connecting: kept
    assert installer.uninstall(merged, OURS_RE, record) == {"env": {**user["env"], "OTEL_PROTOCOL": "grpc"}}
    assert installer.uninstall(installer.merge({}, patch), OURS_RE, installer.added({}, patch)) == {}
