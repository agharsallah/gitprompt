import pytest
from fastapi.testclient import TestClient

from gitprompt.api.app import create_app
from gitprompt.core.config import Settings


@pytest.fixture
def settings(tmp_path, monkeypatch):
    """Everything lives under tmp_path: tests never touch the real ~/.claude or ~/.gitprompt."""
    monkeypatch.delenv("CLAUDE_CONFIG_DIR", raising=False)
    return Settings(home=tmp_path / "gitprompt", user_home=tmp_path / "home")


@pytest.fixture
def claude_settings(settings):
    path = settings.user_home / ".claude" / "settings.json"
    path.parent.mkdir(parents=True)
    return path


@pytest.fixture
def client(settings, monkeypatch):
    monkeypatch.setattr("gitprompt.sources.probe.tool_version", lambda spec: "2.1.316")
    return TestClient(create_app(settings), base_url="http://127.0.0.1:7878")


@pytest.fixture
def hook_payload(tmp_path):
    """Shape documented for UserPromptSubmit at https://code.claude.com/docs/en/hooks (synthetic values)."""
    return {
        "session_id": "abc123",
        "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": str(tmp_path),
        "permission_mode": "default",
        "hook_event_name": "UserPromptSubmit",
        "prompt": "Write a function to calculate the factorial of a number",
    }
