"""The Claude Code hook entry is a shell command. Claude Code shows "hook error" whenever a hook
fails, so the command must exit 0 and print nothing whether GitPrompt is up, down or broken.
These tests run the exact rendered command, the way Claude Code would, against a real socket."""

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


def hook_command(settings) -> str:
    handlers = [h for group in Connection(load_specs(settings)["claude-code"], settings).patch()["hooks"]["Stop"] for h in group["hooks"]]
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


def test_entry_is_async_and_carries_our_url(settings):
    group = Connection(load_specs(settings)["claude-code"], settings).patch()["hooks"]["PostToolUse"][0]
    assert group["hooks"][0]["async"] is True
    assert load_specs(settings)["claude-code"].marker.search(group["hooks"][0]["command"])  # disconnect can find it


def test_delivers_the_event_with_the_token(settings, receiver):
    port, seen, _ = receiver
    result = run(hook_command(replace(settings, port=port)), {"hook_event_name": "Stop", "session_id": "s1"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "")
    path, headers, body = seen[0]
    assert path == "/ingest/claude-code" and headers["Authorization"] == f"Bearer {settings.token()}"
    assert body == {"hook_event_name": "Stop", "session_id": "s1"}


def test_silent_when_gitprompt_is_down(settings):
    result = run(hook_command(replace(settings, port=free_port())), {"hook_event_name": "Stop"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "")


def test_silent_when_gitprompt_answers_500(settings, receiver):
    port, seen, state = receiver
    state["status"] = 500
    result = run(hook_command(replace(settings, port=port)), {"hook_event_name": "Stop"})
    assert (result.returncode, result.stdout, result.stderr) == (0, "", "") and len(seen) == 1
