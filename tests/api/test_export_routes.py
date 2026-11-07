"""GET /api/export/*, end to end through ingest: every kind in every format, with secrets, email
addresses and home paths planted in the synthetic data (ADR-0018)."""

import json
import os
import subprocess
from html.parser import HTMLParser

import pytest

from gitprompt.insights.stories import project_id

AWS_KEY = "AKIA" + "QWERTYUIOPASDFGH"
GITHUB_TOKEN = "ghp_" + "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8"
EMAIL = "jane.doe@example.com"
ACCOUNT_EMAIL = "account.owner@example.org"
ORG_ID = "org-5f1c0ffee"
OTHER_HOME = "/Users/jdoe"
KINDS_WITH_IDS = ("session", "project", "overview", "prompts")
FORMATS = ("md", "html", "csv", "json", "svg")


def git(repo, *args, when=None):
    env = {**os.environ, **({"GIT_COMMITTER_DATE": when, "GIT_AUTHOR_DATE": when} if when else {})}
    author = ["-c", "user.name=Jane Doe", "-c", f"user.email={EMAIL}"]
    subprocess.run(["git", "-C", str(repo), *author, *args], check=True, capture_output=True, env=env)


def otlp(session):
    """A telemetry event whose resource names the account (attributes per the monitoring docs, synthetic)."""

    def attrs(**kv):
        return [{"key": k, "value": {"stringValue": v}} for k, v in kv.items()]

    resource = attrs(**{"service.name": "claude-code", "user.email": ACCOUNT_EMAIL, "organization.id": ORG_ID, "user.id": "u-123456"})
    record = {
        "body": {"stringValue": "claude_code.api_request"},
        "attributes": attrs(**{"event.name": "api_request", "prompt.id": "p1", "session.id": session, "model": "claude-haiku"}),
    }
    return {"resourceLogs": [{"resource": {"attributes": resource}, "scopeLogs": [{"logRecords": [record]}]}]}


@pytest.fixture
def repo(settings):
    path = settings.user_home / "code" / "app"  # inside the home folder, like most projects
    path.mkdir(parents=True)
    git(path, "init", "-b", "main")
    (path / "a.txt").write_text("a\n")
    git(path, "add", ".")
    git(path, "commit", "-m", "initial", when="2026-03-01T08:00:00+00:00")
    return path


@pytest.fixture
def data(client, settings, repo):
    headers = {"Authorization": f"Bearer {settings.token()}"}
    store = client.app.state.store

    def send(name, pid, at, **raw):
        payload = {"hook_event_name": name, "prompt_id": pid, "session_id": "s1", "cwd": str(repo), **raw}
        payload["transcript_path"] = f"{settings.user_home}/.claude/projects/app/s1.jsonl"
        assert client.post("/ingest/claude-code", json=payload, headers=headers).status_code == 204
        table = "prompt" if name == "UserPromptSubmit" else "event"
        store.db.execute(f"update {table} set received_at = ? where id = (select max(id) from {table})", (at,))

    planted = f"deploy with {AWS_KEY} and {GITHUB_TOKEN}, mail {EMAIL}, notes in {OTHER_HOME}/notes.txt and {repo}/src/main.py"
    send("UserPromptSubmit", "p1", "2026-03-02T09:00:00+00:00", prompt=planted)
    send("PostToolUse", "p1", "2026-03-02T09:00:10+00:00", tool_name="Edit", tool_input={"file_path": f"{repo}/a.txt"})
    send(
        "PostToolUse", "p1", "2026-03-02T09:00:20+00:00",
        tool_name="Agent", tool_input={"description": "Look", "subagent_type": "Explore", "prompt": f"check {settings.user_home}/x"},
    )  # fmt: skip
    send("Stop", "p1", "2026-03-02T09:01:00+00:00")
    send("UserPromptSubmit", "p2", "2026-03-02T09:02:00+00:00", prompt=f"<task-notification>done, ping {EMAIL}</task-notification>")
    assert client.post("/ingest/claude-code-otel/v1/logs", json=otlp("s1"), headers=headers).status_code == 200
    (repo / "b.txt").write_text("b\n")
    git(repo, "add", ".")
    git(repo, "commit", "-m", "Add b", when="2026-03-02T09:00:40+00:00")
    return client


def params(kind, repo, **extra):
    ids = {"session": {"id": "s1"}, "project": {"id": project_id(str(repo))}}
    return {**ids.get(kind, {}), **({"origin": ["typed", "slash_command"]} if kind in ("overview", "prompts") else {}), **extra}


def fetch(client, kind, repo, **extra):
    response = client.get(f"/api/export/{kind}", params=params(kind, repo, **extra))
    assert response.status_code == 200, response.text
    return response


NEVER = (AWS_KEY, GITHUB_TOKEN, EMAIL, ACCOUNT_EMAIL, ORG_ID, OTHER_HOME)


@pytest.mark.parametrize("kind", KINDS_WITH_IDS)
@pytest.mark.parametrize("fmt", FORMATS)
def test_defaults_leave_out_secrets_emails_paths_and_text(data, settings, repo, kind, fmt):
    body = fetch(data, kind, repo, format=fmt).text
    for value in (*NEVER, str(settings.user_home), "deploy with", "Jane Doe", "u-123456"):
        assert value not in body, (kind, fmt, value)


@pytest.mark.parametrize("kind", KINDS_WITH_IDS)
@pytest.mark.parametrize("fmt", ("md", "html", "csv", "json"))
def test_secrets_emails_and_home_paths_stay_hidden_with_prompt_text(data, settings, repo, kind, fmt):
    body = fetch(data, kind, repo, format=fmt, prompt_text=True, drop_raw=False).text
    for value in (*NEVER, str(settings.user_home)):
        assert value not in body, (kind, fmt, value)
    if kind != "overview":  # the overview has no prompt text
        assert "deploy with [hidden AWS access key]" in body and "[hidden email]" in body
    if kind in ("session", "prompts"):  # a project story shows the start of each prompt only
        assert "src/main.py" in body and "~/notes.txt" in body


def test_each_option_puts_back_what_it_hides(data, settings, repo):
    paths = fetch(data, "session", repo, format="json", relative_paths=False).text
    assert f"{repo}/a.txt" in paths
    identity = fetch(data, "session", repo, format="md", drop_identity=False).text
    assert ACCOUNT_EMAIL in identity and ORG_ID in identity
    raw = json.loads(fetch(data, "prompts", repo, format="json", drop_raw=False, prompt_text=True).text)
    assert raw["rows"][-1]["raw"]["hook_event_name"] == "UserPromptSubmit"
    assert AWS_KEY not in json.dumps(raw)  # secrets are hidden whatever the options


class Resources(HTMLParser):
    """Every element and attribute that could make a browser fetch something."""

    def __init__(self):
        super().__init__()
        self.tags: list[str] = []
        self.urls: list[str] = []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        for name, value in attrs:
            if name in ("src", "href", "srcset", "action", "poster", "data", "xlink:href") and value:
                self.urls.append(value)


@pytest.mark.parametrize("kind", KINDS_WITH_IDS)
def test_html_is_self_contained(data, repo, kind):
    response = fetch(data, kind, repo, format="html", prompt_text=True)
    page = response.text
    parser = Resources()
    parser.feed(page)
    assert not {"script", "link", "img", "iframe", "object", "embed", "video", "audio", "source", "base"} & set(parser.tags)
    assert parser.urls == []
    assert "http://" not in page and "https://" not in page and "url(" not in page and "@import" not in page
    assert "default-src 'none'" in page and response.headers["content-security-policy"].startswith("default-src 'none'")
    assert "<svg" in page and "prefers-color-scheme: dark" in page and "@media print" in page


def test_a_single_chart_and_the_chart_list(data, repo):
    charts = data.get("/api/export/session/charts", params={"id": "s1"}).json()
    assert {c["id"] for c in charts} >= {"outcomes", "tools"}
    svg = fetch(data, "session", repo, format="svg", chart="tools")
    assert svg.headers["content-type"].startswith("image/svg+xml") and "Tool calls by kind" in svg.text
    assert data.get("/api/export/session", params={"id": "s1", "format": "svg", "chart": "nope"}).status_code == 200  # the first chart


def test_download_names_the_file(data, repo):
    response = fetch(data, "project", repo, format="md", download=True)
    assert response.headers["content-disposition"].startswith('attachment; filename="gitprompt-project-app-')
    assert response.headers["content-disposition"].endswith('.md"')


def test_errors(data):
    assert data.get("/api/export/session").status_code == 422  # no id
    assert data.get("/api/export/session", params={"id": "nope"}).status_code == 404
    assert data.get("/api/export/nothing").status_code == 422
