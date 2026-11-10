"""The export pipeline without the API: the redaction pass, the builders and every format, on
synthetic data with planted secrets, emails and home paths (ADR-0018)."""

import csv
import io
import json

import pytest

from gitprompt import export
from gitprompt.export.formats import FORMATS, Export, register
from gitprompt.export.redact import Options, redact

HOME = "/Users/sam"
REPO = f"{HOME}/code/shop"
SECRET = "sk-ant-" + "api03-abcdefghijklmnopqrstuvwxyz0123"
EMAIL = "sam@example.com"


def story() -> dict:
    """A session story as /api/stories/session returns it (synthetic)."""
    chapter = {
        "kind": "chapter", "id": 7, "turn": 1, "received_at": "2026-03-02T09:00:00+00:00", "origin": "typed",
        "text": f"use {SECRET} and write to {REPO}/src/app.py, then mail {EMAIL}", "clipped": False,
        "slash_command": None, "branch": "main", "outcome": "done", "response_s": 42.0, "cost_usd": 0.12, "tokens": 900,
        "tool_calls": 3, "tool_failures": 0, "tools": {"write": 2, "read": 1}, "files_written": 1,
        "files": [f"{REPO}/src/app.py", f"{HOME}/.zshrc"], "subagents": 0, "git_commits": 1,
        "commits": [{"hash": "abc1234def", "subject": "Add app", "author": "Sam Doe", "time": "2026-03-02T09:00:30+00:00",
                     "files": 1, "insertions": 3, "deletions": 0}],
        "diffstat": {"commits": 1, "files": 1, "insertions": 3, "deletions": 0}, "retry_of": None,
    }  # fmt: skip
    markers = {"kind": "markers", "items": [
        {"kind": "task_notification", "id": 8, "received_at": "2026-03-02T09:02:00+00:00", "label": None, "detail": None,
         "excerpt": f"done, see {HOME}/Desktop/log.txt", "clipped": False},
    ]}  # fmt: skip
    return {
        "session": "s-0001-0002", "title": "Wire the shop", "source": "claude-code",
        "project": {"id": "x", "repo": REPO, "name": "shop", "git": True}, "branches": ["main"],
        "started_at": "2026-03-02T09:00:00+00:00", "ended_at": "2026-03-02T09:03:00+00:00",
        "totals": {"chapters": 1, "markers": 1, "cost_usd": 0.12, "tool_calls": 3, "files_written": 1,
                   "outcomes": {"done": 1}, "retries": 0, "diffstat": chapter["diffstat"]},
        "items": [chapter, markers],
    }  # fmt: skip


def data() -> dict:
    environment = {"app.version": "2.1.0", "user.email": EMAIL, "organization.id": "org-42", "user.account_uuid": "acc-9"}
    return {"generated_at": "2026-03-03T10:00:00+00:00", "story": story(), "environment": environment}


def everything(options: Options) -> dict[str, str]:
    prepared = export.prepare("session", data(), options, HOME)
    return {fmt: export.render(prepared, fmt) for fmt in FORMATS}


@pytest.mark.parametrize("options", [Options(), Options(prompt_text=True), Options(prompt_text=True, drop_raw=False)])
def test_no_format_shows_secrets_emails_or_home_paths(options):
    for fmt, body in everything(options).items():
        for value in (SECRET, EMAIL, HOME, "org-42", "acc-9", "Sam Doe"):
            assert value not in body, (fmt, value)


def test_defaults_are_figures_only():
    for fmt, body in everything(Options()).items():
        assert "write to" not in body and "Wire the shop" not in body and "done, see" not in body, fmt
    md = everything(Options())["md"]
    assert "Session s-0001-0" in md and "src/app.py" in md and "~/.zshrc" in md  # paths stay, made relative


def test_prompt_text_is_quoted_with_secrets_and_emails_masked():
    md = everything(Options(prompt_text=True))["md"]
    assert "use [hidden Anthropic API key] and write to src/app.py, then mail [hidden email]" in md
    assert "You typed:" in md and "done, see ~/Desktop/log.txt" in md and "# Wire the shop" in md


def test_redact_rules():
    value = {"repo": REPO, "files": [f"{REPO}/a.py", REPO, f"{REPO}x/b.py"], "raw": {"k": 1}, "text": "t", "user.id": "u1"}
    assert redact(value, Options()) == {"repo": "shop", "files": ["a.py", "shop", "~/code/shopx/b.py"]}
    kept = redact(value, Options(prompt_text=True, relative_paths=False, drop_identity=False, drop_raw=False))
    assert kept == value
    assert redact({"note": f"key={SECRET}"}, Options(relative_paths=False, drop_identity=False)) == {
        "note": "key=[hidden Anthropic API key]"
    }
    assert redact("other home /home/alex/x and /Users/bo", Options()) == "other home ~/x and ~"
    assert redact("~/.claude/projects/-Users-bo-code-app/m.md", Options()) == "~/.claude/projects/…/m.md"
    assert redact("starts the key sk-ant-api03-abcd…", Options()) == "starts the key …"  # a cut secret can't be matched


def test_csv_rows_and_spreadsheet_formulas():
    prepared = export.prepare("session", data(), Options(prompt_text=True), HOME)
    rows = list(csv.DictReader(io.StringIO(export.render(prepared, "csv"))))
    assert [r["kind"] for r in rows] == ["chapter", "task_notification"] and rows[0]["cost_usd"] == "0.12"
    evil = Export(prepared.report, [{"a": "=HYPERLINK(1)", "b": ["x"]}])
    assert FORMATS["csv"].render(evil) == 'a,b\n\'=HYPERLINK(1),"[""x""]"\n'


def test_json_states_its_options():
    document = json.loads(everything(Options())["json"])
    assert document["options"] == Options().as_dict() and document["kind"] == "session" and len(document["rows"]) == 2


def test_markdown_keeps_prompt_markdown_inert():
    d = data()
    d["story"]["items"][0]["text"] = "![x](https://tracker.invalid/p.png) ```code```"
    md = export.render(export.prepare("session", d, Options(prompt_text=True), HOME), "md")
    assert "````text\n![x](https://tracker.invalid/p.png) ```code```\n````" in md  # fenced: no image is loaded


def test_html_escapes_text():
    d = data()
    d["story"]["items"][0]["text"] = "<img src=x onerror=alert(1)>"
    page = export.render(export.prepare("session", d, Options(prompt_text=True), HOME), "html")
    assert "<img" not in page and "&lt;img src=x" in page


def test_a_new_format_needs_no_other_change():
    @register("txt", "Plain text", "text/plain", ".txt")
    def plain(e: Export) -> str:
        return "\n".join(s.title for s in e.report.sections)

    try:
        assert export.render(export.prepare("session", data(), Options(), HOME), "txt").startswith("Summary\nEnvironment\nPrompt 1")
    finally:
        FORMATS.pop("txt")


def test_rows_name_the_app_a_prompt_was_typed_in():
    from gitprompt.export.build import fact_columns, tool

    desktop = {"id": "desktop", "name": "Desktop app", "icon": "app-window"}
    facts = {"connector": {"id": "claude-code", "label": "Claude Code"}, "surface": desktop}
    assert tool(facts) == "Claude Code (Desktop app)" and fact_columns(facts)["surface"] == "desktop"
    assert tool({"connector": {"label": "Codex"}, "surface": None}) == "Codex"
