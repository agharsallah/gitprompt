import json
from pathlib import Path

from gitprompt.insights import derivations
from gitprompt.insights.redaction import find_secrets, redact
from gitprompt.sources.spec import load_specs, tool_kind

COPILOT_FIXTURES = Path(__file__).parents[1] / "sources" / "fixtures" / "copilot-cli"


def test_prompt_stats():
    stats = derivations.prompt_stats("/review @src/app.py see https://example.com/x?\n```py\nx\n```")
    assert stats["slash_command"] == "/review" and stats["at_mentions"] == ["src/app.py"]
    assert stats["urls"] == ["https://example.com/x?"] and stats["code_blocks"] == 1 and stats["lines"] == 4
    assert "secrets" not in stats


def test_secrets_are_named_and_redacted_without_changing_structure():
    key = "AKIA" + "ABCDEFGHIJKLMNOP"
    assert find_secrets(f"use {key} please") == ["AWS access key"]
    assert redact({"a": [f"k={key}"], "n": 1}) == {"a": ["k=[hidden AWS access key]"], "n": 1}


def test_turn_summary_from_events(settings):
    prompt = {"received_at": "2026-11-03T10:00:00+00:00", "turn": 2, "prev_at": "2026-11-03T09:59:00+00:00"}

    def event(name, **raw):
        return {"event": name, "received_at": "2026-11-03T10:00:30+00:00", "raw": raw}

    events = [
        event("PostToolUse", tool_name="Write", tool_input={"file_path": "/r/a.py", "content": "x"}, duration_ms=1500),
        event("PostToolUse", tool_name="Bash", tool_input={"command": "pytest"}),
        event("api_request", model="claude-haiku", input_tokens="10", output_tokens=5, cost_usd=0.01),
        event("Stop", last_assistant_message="Done."),
    ]
    summary = derivations.turn(prompt, events, load_specs(settings)["claude-code"].tool_kinds)
    assert summary["since_previous_prompt_s"] == 60 and summary["response_s"] == 30 and summary["response"] == "Done."
    assert summary["tool_calls"] == {"Write": 1, "Bash": 1} and summary["files_written"] == ["/r/a.py"]
    assert summary["bash_commands"] == ["pytest"] and summary["tool_time_s"] == 1.5
    assert summary["input_tokens"] == 10 and summary["cost_usd"] == 0.01 and summary["models"] == {"claude-haiku": 1}
    assert summary["tool_kinds"] == {"write": 1, "shell": 1}
    assert summary["model_usage"] == {
        "claude-haiku": {
            "api_requests": 1,
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_tokens": 0,
            "cache_creation_tokens": 0,
            "cost_usd": 0.01,
        }
    }


def test_tool_kinds_come_from_the_spec(settings):
    kinds = load_specs(settings)["claude-code"].tool_kinds
    names = ("Grep", "MultiEdit", "Bash", "WebSearch", "mcp__gh__x", "Agent", "Nope", None)
    assert [tool_kind(kinds, name) for name in names] == ["read", "write", "shell", "web", "mcp", "agent", "other", "other"]
    assert tool_kind({"mcp__*": "mcp", "mcp__x": "web"}, "mcp__x") == "web"  # an exact name wins over a pattern


def test_a_turn_without_a_spec_has_only_other_tools():
    events = [
        {"event": "PostToolUse", "received_at": "2026-11-03T10:00:01+00:00", "raw": {"tool_name": "Bash", "tool_input": {"command": "ls"}}}
    ]
    summary = derivations.turn({"received_at": "2026-11-03T10:00:00+00:00"}, events)
    assert summary["tool_kinds"] == {"other": 1} and "bash_commands" not in summary


def test_copilot_tool_calls_get_their_kinds_files_and_commands(settings):
    payloads = [json.loads(p.read_text())["payload"] for p in sorted(COPILOT_FIXTURES.glob("*.json"))]
    events = [
        {"event": p["hook_event_name"], "received_at": p["timestamp"], "raw": p}
        for p in payloads
        if p.get("hook_event_name") in derivations.TOOL_RESULTS
    ]
    events.append(
        {
            "event": "PostToolUse",
            "received_at": "2026-11-07T10:00:20.000Z",
            "raw": {"tool_name": "bash", "tool_input": {"command": "git commit -m x"}},
        }
    )
    summary = derivations.turn({"received_at": "2026-11-07T10:00:00.000Z"}, events, load_specs(settings)["copilot-cli"].tool_kinds)
    assert summary["tool_kind_of"] == {"bash": "shell", "edit": "write", "view": "read"}
    assert summary["files_written"] == ["client.py"] and summary["files_read"] == ["README.md"]
    assert summary["bash_commands"] == ["ls src", "git commit -m x"] and summary["git_commits"] == 1
