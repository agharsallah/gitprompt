from gitprompt.insights import derivations
from gitprompt.insights.redaction import find_secrets, redact


def test_prompt_stats():
    stats = derivations.prompt_stats("/review @src/app.py see https://example.com/x?\n```py\nx\n```")
    assert stats["slash_command"] == "/review" and stats["at_mentions"] == ["src/app.py"]
    assert stats["urls"] == ["https://example.com/x?"] and stats["code_blocks"] == 1 and stats["lines"] == 4
    assert "secrets" not in stats


def test_secrets_are_named_and_redacted_without_changing_structure():
    key = "AKIA" + "ABCDEFGHIJKLMNOP"
    assert find_secrets(f"use {key} please") == ["AWS access key"]
    assert redact({"a": [f"k={key}"], "n": 1}) == {"a": ["k=[hidden AWS access key]"], "n": 1}


def test_turn_summary_from_events():
    prompt = {"received_at": "2026-11-03T10:00:00+00:00", "turn": 2, "prev_at": "2026-11-03T09:59:00+00:00"}

    def event(name, **raw):
        return {"event": name, "received_at": "2026-11-03T10:00:30+00:00", "raw": raw}

    events = [
        event("PostToolUse", tool_name="Write", tool_input={"file_path": "/r/a.py", "content": "x"}, duration_ms=1500),
        event("PostToolUse", tool_name="Bash", tool_input={"command": "pytest"}),
        event("api_request", model="claude-haiku", input_tokens="10", output_tokens=5, cost_usd=0.01),
        event("Stop", last_assistant_message="Done."),
    ]
    summary = derivations.turn(prompt, events)
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


def test_tool_kinds():
    kinds = {name: derivations.tool_kind(name) for name in ("Grep", "MultiEdit", "Bash", "WebSearch", "mcp__gh__x", "Agent", "Nope", None)}
    assert list(kinds.values()) == ["read", "write", "shell", "web", "mcp", "agent", "other", "other"]
