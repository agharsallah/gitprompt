"""The Claude Code and Codex hook entries are shell commands. Both tools report a failing hook
(Claude Code shows "hook error"; Codex adds a hook's stdout to the conversation on some events), so
the command must exit 0 and print nothing whether GitPrompt is up, down or broken. These tests run
the exact rendered command, the way the tools would, against a real socket."""

import json
import shutil
import socket
import subprocess
import threading
from dataclasses import replace
from http.server import BaseHTTPRequestHandler, HTTPServer

import pytest

from gitprompt.sources.connection import Connection
from gitprompt.sources.spec import load_specs

pytestmark = pytest.mark.skipif(shutil.which("curl") is None, reason="the forwarder uses curl")


TOOLS = ["claude-code", "codex"]  # same hook shape: event -> groups -> handlers with a `command`


def hook_command(settings, tool: str = "claude-code") -> str:
    handlers = [h for group in Connection(load_specs(settings)[tool], settings).patch()["hooks"]["Stop"] for h in group["hooks"]]
    assert [h["type"] for h in handlers] == ["command"]
    return handlers[0]["command"]


def run(command: str, payload: dict) -> subprocess.CompletedProcess:
    return subprocess.run(["sh", "-c", command], input=json.dumps(payload), capture_output=True, text=True, timeout=10)


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@pytest.fixture
def receiver():
    """A stand-in receiver that records each call and answers with `status`."""
    seen: list[tuple[str, dict, dict]] = []
    state = {"status": 204}

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers["Content-Length"]))
            seen.append((self.path, dict(self.headers), json.loads(body)))
            self.send_response(state["status"])
            self.end_headers()
            if state["status"] >= 400:
                self.wfile.write(b"Internal Server Error")

        def log_message(self, *args):
            pass

    server = HTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield server.server_address[1], seen, state
    server.shutdown()


@pytest.mark.parametrize("tool", TOOLS)
def test_entry_is_async_and_carries_our_url(settings, tool):
    group = Connection(load_specs(settings)[tool], settings).patch()["hooks"]["PostToolUse"][0]
    assert group["hooks"][0]["async"] is True
    assert load_specs(settings)[tool].marker.search(group["hooks"][0]["command"])  # disconnect can find it


@pytest.mark.parametrize("tool", TOOLS)
def test_delivers_the_event_with_the_token(settings, receiver, tool):
    port, seen, _ = receiver
    result = run(hook_command(replace(settings, port=port), tool), {"hook_event_name": "Stop", "session_id": "s1"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "")
    path, headers, body = seen[0]
    assert path == f"/ingest/{tool}" and headers["Authorization"] == f"Bearer {settings.token()}"
    assert body == {"hook_event_name": "Stop", "session_id": "s1"}


@pytest.mark.parametrize("tool", TOOLS)
def test_silent_when_gitprompt_is_down(settings, tool):
    result = run(hook_command(replace(settings, port=free_port()), tool), {"hook_event_name": "Stop"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "")


def test_silent_when_gitprompt_answers_500(settings, receiver):
    port, seen, state = receiver
    state["status"] = 500
    result = run(hook_command(replace(settings, port=port)), {"hook_event_name": "Stop"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "") and len(seen) == 1


# --- Cursor: flat handlers (event -> [{command, timeout}]); each must answer with JSON that changes nothing ---


def cursor_command(settings, event: str) -> str:
    (handler,) = Connection(load_specs(settings)["cursor"], settings).patch()["hooks"][event]
    assert set(handler) == {"command", "timeout"}  # never failClosed, never a prompt-type hook
    return handler["command"]


# The answer Cursor reads for each subscribed event: let the prompt through, and nothing else.
CURSOR_ANSWERS = {"beforeSubmitPrompt": {"continue": True}, "stop": {}, "postToolUse": {}, "sessionStart": {}, "afterAgentResponse": {}}


@pytest.mark.parametrize(("event", "answer"), CURSOR_ANSWERS.items())
def test_cursor_answers_without_changing_anything(settings, receiver, event, answer):
    port, seen, _ = receiver
    result = run(cursor_command(replace(settings, port=port), event), {"hook_event_name": event, "generation_id": "g1"})
    assert (result.returncode, json.loads(result.stdout), result.stderr) == (0, answer, "")
    assert seen[0][0] == "/ingest/cursor" and seen[0][2]["generation_id"] == "g1"


@pytest.mark.parametrize("event", CURSOR_ANSWERS)
def test_cursor_lets_the_prompt_through_when_gitprompt_is_down(settings, event):
    result = run(cursor_command(replace(settings, port=free_port()), event), {"hook_event_name": event})
    assert (result.returncode, json.loads(result.stdout), result.stderr) == (0, CURSOR_ANSWERS[event], "")


def test_cursor_subscribes_to_no_permission_hook(settings):
    events = set(Connection(load_specs(settings)["cursor"], settings).patch()["hooks"])
    assert not events & {"preToolUse", "beforeShellExecution", "beforeMCPExecution", "beforeReadFile", "subagentStart", "beforeTabFileRead"}
