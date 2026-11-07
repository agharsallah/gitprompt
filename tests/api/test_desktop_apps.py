"""Prompts typed in desktop apps and IDEs land in the right session and project, carry their surface,
and can be filtered, grouped and checked (ADR-0022). Real temporary git repositories, synthetic
payloads."""

import subprocess

import pytest

from gitprompt.insights.captures import recent
from gitprompt.insights.indexer import Indexer
from gitprompt.sources.spec import load_specs
from gitprompt.storage.stats import latest
from gitprompt.storage.store import Store


def git(repo, *args):
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path):
    """A repository with one commit and a linked worktree on its own branch."""
    main = tmp_path / "shop"
    main.mkdir()
    git(main, "init", "-b", "main")
    (main / "a.txt").write_text("a")
    git(main, "add", ".")
    git(main, "commit", "-m", "first")
    (main / "pkg").mkdir()
    git(main, "worktree", "add", "-b", "feature", str(main / ".claude" / "worktrees" / "feature"))
    return main


@pytest.fixture
def ingest(client, settings):
    auth = {"Authorization": f"Bearer {settings.token()}"}

    def send(source: str, payload: dict) -> None:
        assert client.post(f"/ingest/{source}", json=payload, headers=auth).status_code < 300

    return send


def claude_prompt(cwd, n: int, session: str = "s-desk") -> dict:
    return {"hook_event_name": "UserPromptSubmit", "session_id": session, "prompt_id": f"p{n}", "cwd": str(cwd), "prompt": f"task {n}"}


def otlp(prompt_id: str, **resource) -> dict:
    """A telemetry user_prompt record, as Claude Code exports it (OTLP/HTTP JSON)."""

    def attrs(values: dict) -> list[dict]:
        return [{"key": k, "value": {"stringValue": v}} for k, v in values.items()]

    record = {"attributes": attrs({"event.name": "user_prompt", "prompt.id": prompt_id})}
    resource = attrs(resource)
    return {"resourceLogs": [{"resource": {"attributes": resource}, "scopeLogs": [{"logRecords": [record]}]}]}


def cursor_prompt(roots: list[str], gen: str = "g1") -> dict:
    return {
        "hook_event_name": "beforeSubmitPrompt",
        "conversation_id": "conv-1",
        "generation_id": gen,
        "workspace_roots": roots,
        "model": "gpt-6",
        "cursor_version": "3.4.1",
        "user_email": "someone@example.test",
        "prompt": "rename the cart",
    }


def test_a_worktree_prompt_belongs_to_the_main_repository(client, ingest, repo):
    worktree = repo / ".claude" / "worktrees" / "feature"
    ingest("claude-code", claude_prompt(worktree, 1))
    ingest("claude-code", claude_prompt(repo / "pkg", 2))  # a subfolder of the main checkout
    first, second = sorted(client.get("/api/prompts").json(), key=lambda p: p["id"])
    assert first["context"]["git_root"] == str(repo.resolve()) == second["context"]["git_root"]
    assert first["context"]["git_worktree_root"] == str(worktree.resolve()) and first["context"]["git_branch"] == "feature"
    assert "git_worktree_root" not in second["context"]
    assert [g["key"] for g in client.get("/api/stats", params={"group_by": "repo"}).json()["groups"]] == [str(repo.resolve())]


def test_a_folder_without_git_is_its_own_project(client, ingest, tmp_path):
    notes = tmp_path / "notes"
    notes.mkdir()
    ingest("claude-code", claude_prompt(notes, 1))
    (p,) = client.get("/api/prompts").json()
    assert "git_root" not in p["context"]
    assert client.get("/api/stats", params={"group_by": "repo"}).json()["groups"][0]["key"] == str(notes)


def test_a_multi_root_cursor_workspace_uses_its_first_root(client, ingest, repo, tmp_path):
    ingest("cursor", cursor_prompt([str(repo / "pkg"), str(tmp_path)]))
    (p,) = client.get("/api/prompts").json()
    assert p["context"]["git_root"] == str(repo.resolve())
    assert p["fields"]["workspace_roots"] == [str(repo / "pkg"), str(tmp_path)]
    assert p["fields"]["session_id"] == "conv-1" and p["facts"]["connector"]["label"] == "Cursor"
    assert p["facts"]["surface"] is None  # Cursor doesn't say whether the IDE or the CLI sent it
    assert p["facts"]["tool_version"] == "3.4.1"  # the version Cursor reported with the prompt
    assert "user_email" not in client.get(f"/api/prompts/{p['id']}").json()["raw"]


def test_surface_is_a_fact_a_filter_and_a_grouping(client, ingest, tmp_path):
    for n, entrypoint in ((1, "cli"), (2, "claude-vscode"), (3, None)):
        ingest("claude-code", claude_prompt(tmp_path, n, session=f"s{n}"))
        if entrypoint:
            ingest("claude-code-otel", otlp(f"p{n}", **{"app.entrypoint": entrypoint}))
    ingest("claude-code", claude_prompt(tmp_path, 4, session="s4"))
    ingest("claude-code-otel", otlp("p4", **{"service.name": "claude-code-desktop", "app.entrypoint": "claude-desktop"}))

    surfaces = {p["typed_text"]: p["facts"]["surface"] for p in client.get("/api/prompts").json()}
    assert surfaces["task 1"]["id"] == "cli" and surfaces["task 2"]["name"] == "VS Code" and surfaces["task 3"] is None
    assert surfaces["task 4"] == {"id": "desktop", "name": "Desktop app", "icon": "app-window"}
    assert [p["typed_text"] for p in client.get("/api/prompts", params={"surface": ["desktop", "cli"]}).json()] == ["task 4", "task 1"]
    groups = {g["key"]: (g["prompts"], g["detail"]) for g in client.get("/api/stats", params={"group_by": "surface"}).json()["groups"]}
    assert groups == {"cli": (1, "CLI"), "vscode": (1, "VS Code"), "desktop": (1, "Desktop app"), None: (1, None)}
    story = client.get("/api/stories/session/s4").json()
    assert story["surface"]["id"] == "desktop"


def test_recent_captures_show_the_newest_prompt_per_connector_and_surface(client, ingest, settings, repo):
    ingest("claude-code", claude_prompt(repo, 1, session="old"))
    ingest("claude-code-otel", otlp("p1", **{"app.entrypoint": "cli"}))
    ingest("claude-code", claude_prompt(repo / ".claude" / "worktrees" / "feature", 2, session="new"))
    ingest("claude-code-otel", otlp("p2", **{"app.entrypoint": "cli"}))
    ingest("cursor", cursor_prompt([str(repo)]))

    rows = client.get("/api/sources/recent").json()
    assert [(r["connector"]["id"], r["surface"] and r["surface"]["id"]) for r in rows] == [("cursor", None), ("claude-code", "cli")]
    claude = rows[1]
    assert (claude["session"], claude["prompts"], claude["branch"]) == ("new", 2, "feature")
    assert claude["project"] == {"repo": str(repo.resolve()), "name": "shop"} and claude["worktree"].endswith("feature")
    assert claude["signals"] == {"app.entrypoint": "cli"} and rows[0]["model"] == "gpt-6"


def test_latest_groups_by_connector_and_surface(settings):
    store = Store(settings.db_path)
    specs = load_specs(settings)
    for n, source in enumerate(("codex", "codex", "copilot-cli")):
        store.add_prompt(source, {"prompt_id": f"t{n}", "session_id": f"s{n}", "event": "UserPromptSubmit", "prompt": "x"}, {}, {})
    Indexer(store, specs).ensure()
    assert [(r["source"], r["prompts"], r["session"]) for r in latest(store.db)] == [("copilot-cli", 1, "s2"), ("codex", 2, "s1")]
    assert [r["signals"] for r in recent(store, specs)] == [{}, {}]
    assert latest(Store(settings.home / "empty.db").db) == []


def test_sources_list_what_each_connector_captures(client):
    sources = {s["id"]: s for s in client.get("/api/sources").json()}
    claude = sources["claude-code"]
    assert {c["id"] for c in claude["captures"]} == {"cli", "vscode", "desktop", "sdk"}
    assert "Claude Desktop chat" in claude["cannot_capture"]
    assert {c["id"] for c in sources["codex"]["captures"]} >= {"cli", "desktop"}
    assert sources["cursor"]["captures"][0]["name"] == "Cursor" and "Cursor CLI" in sources["cursor"]["cannot_capture"]


def test_gitprompt_check_prints_each_connector_and_app(client, ingest, settings, tmp_path, capsys):
    from gitprompt.cli import check

    ingest("claude-code", claude_prompt(tmp_path, 1))
    ingest("claude-code-otel", otlp("p1", **{"app.entrypoint": "claude-vscode"}))
    check(settings, None)
    out = capsys.readouterr().out
    assert "Claude Code: Hooks not connected, Telemetry not connected (not detected on this machine)" in out
    assert "VS Code" in out and "app.entrypoint=claude-vscode" in out and "1 prompts" in out
    assert "Cursor: Hooks not connected (not detected on this machine)" in out and "no prompt captured yet" in out
    assert "not capturable: Claude Desktop chat" in out
