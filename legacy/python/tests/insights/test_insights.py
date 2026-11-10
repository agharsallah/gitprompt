import json
import re
from pathlib import Path

from gitprompt.insights import derivations
from gitprompt.insights.redaction import find_secrets, redact
from gitprompt.sources.spec import load_specs, tool_kind

COPILOT_FIXTURES = Path(__file__).parents[1] / "sources" / "fixtures" / "copilot-cli"
CODEX_FIXTURES = COPILOT_FIXTURES.parent / "codex"


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
    summary = derivations.turn(prompt, events, load_specs(settings)["claude-code"])
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
    summary = derivations.turn({"received_at": "2026-11-07T10:00:00.000Z"}, events, load_specs(settings)["copilot-cli"])
    assert summary["tool_kind_of"] == {"bash": "shell", "edit": "write", "view": "read"}
    assert summary["files_written"] == ["client.py"] and summary["files_read"] == ["README.md"]
    assert summary["bash_commands"] == ["ls src", "git commit -m x"] and summary["git_commits"] == 1


def test_codex_turns_get_files_from_patches_and_the_answer(settings):
    """apply_patch names its files inside the patch: the spec's `tool_files` pattern finds them."""
    payloads = [json.loads(p.read_text())["payload"] for p in sorted(CODEX_FIXTURES.glob("*.json"))]
    turn = next(p["turn_id"] for p in payloads if p["hook_event_name"] == "UserPromptSubmit")
    at = "2026-11-07T10:00:09+00:00"
    events = [{"event": p["hook_event_name"], "received_at": at, "raw": p} for p in payloads if p.get("turn_id") == turn]
    codex = load_specs(settings)["codex"]
    summary = derivations.turn({"received_at": "2026-11-07T10:00:00+00:00"}, events, codex)
    assert summary["tool_kind_of"] == {"Bash": "shell", "apply_patch": "write", "collaborationspawn_agent": "agent"}
    assert summary["files_written"] == ["/work/app/upload.py"]
    assert summary["bash_commands"] == ["pytest -q tests/test_upload.py", "cat missing.txt"]
    assert summary["response"].startswith("Added a retry") and summary["ended_by"] == "Stop"
    # PostModelRequest: input tokens without the cached ones, as Anthropic counts them; no cost.
    usage = {
        "api_requests": 1,
        "input_tokens": 3000,
        "output_tokens": 300,
        "cache_read_tokens": 9000,
        "cache_creation_tokens": 0,
        "cost_usd": 0,
    }
    assert summary["model_usage"] == {"gpt-5-codex": usage} and summary["reasoning_tokens"] == 120
    assert summary["input_tokens"] == 3000 and "cost_usd" not in summary


def test_codex_interrupted_turn_and_failed_model_call(settings):
    payloads = [json.loads(p.read_text())["payload"] for p in sorted(CODEX_FIXTURES.glob("*.json"))]
    turn = "0190a1b2-0000-7000-8000-0000000000a3"
    events = [
        {"event": p["hook_event_name"], "received_at": "2026-11-07T10:00:05+00:00", "raw": p}
        for p in payloads
        if p.get("turn_id") == turn and p["hook_event_name"] != "UserPromptSubmit"
    ]
    summary = derivations.turn({"received_at": "2026-11-07T10:00:00+00:00"}, events, load_specs(settings)["codex"])
    assert summary["ended_by"] == "Interrupt" and summary["response_s"] == 5
    assert summary["api_requests"] == 1 and summary["api_errors"] == 1 and "input_tokens" not in summary


def test_session_starts_list_how_it_began_and_each_resume():
    payloads = [json.loads(p.read_text())["payload"] for p in sorted(CODEX_FIXTURES.glob("*.json"))]
    events = [{"event": p["hook_event_name"], "received_at": "2026-11-07T10:00:00+00:00", "raw": p} for p in payloads]
    assert derivations.session(events)["starts"] == ["startup", "resume"]


def test_files_of_prefers_a_path_input_and_ignores_unknown_tools():
    pattern = {"apply_patch": re.compile(r"(?m)^\*\*\* (?:Add|Update|Delete) File: (.+)$")}
    patch = "*** Begin Patch\n*** Add File: a.txt\n+x\n*** Delete File: b.txt\n*** End Patch"
    assert derivations.files_of("apply_patch", {"command": patch}, pattern) == ["a.txt", "b.txt"]
    assert derivations.files_of("apply_patch", {"path": "c.txt", "command": patch}, pattern) == ["c.txt"]
    assert derivations.files_of("Bash", {"command": patch}, pattern) == []
    assert derivations.files_of("apply_patch", "not an object", pattern) == []
