"""Where a prompt was typed (insights/surface.py): the spec's surface rules, first match wins,
unknown stays None."""

import pytest

from gitprompt.insights.surface import catalog, signals, surface_of
from gitprompt.sources.spec import load_specs, parse_spec


def prompt(raw: dict | None = None) -> dict:
    return {"prompt": "hi", "fields": {}, "raw": raw or {}}


def telemetry(**attrs) -> dict:
    return {"event": "user_prompt", "raw": {"event.name": "user_prompt", **attrs}}


@pytest.fixture
def claude(settings):
    return load_specs(settings)["claude-code"]


@pytest.mark.parametrize(
    ("attrs", "surface"),
    [
        ({"app.entrypoint": "cli", "service.name": "claude-code"}, "cli"),
        ({"app.entrypoint": "claude-vscode"}, "vscode"),
        ({"app.entrypoint": "sdk-cli"}, "sdk"),
        ({"app.entrypoint": "sdk-ts"}, "sdk"),
        ({"app.entrypoint": "claude-desktop"}, "desktop"),
        # The documented desktop signal wins over whatever entrypoint the Code tab reports.
        ({"service.name": "claude-code-desktop", "app.entrypoint": "sdk-ts"}, "desktop"),
        ({"app.entrypoint": "something-new"}, None),
        ({}, None),
    ],
)
def test_claude_code_surface_comes_from_its_telemetry(claude, attrs, surface):
    assert surface_of(prompt(), [telemetry(**attrs)], claude) == surface


def test_without_telemetry_the_surface_is_unknown(claude):
    assert surface_of(prompt({"hook_event_name": "UserPromptSubmit"}), [], claude) is None


def test_a_payload_rule_needs_no_event(claude):
    """Cursor running Claude Code's hooks adds its version to the payload."""
    assert surface_of(prompt({"cursor_version": "3.4.1"}), [], claude) == "cursor"


def test_tools_without_surface_rules_never_guess(settings):
    specs = load_specs(settings)
    for tool in ("codex", "copilot-cli", "cursor"):
        assert surface_of(prompt({"cwd": "/w", "model": "m"}), [telemetry(**{"app.entrypoint": "cli"})], specs[tool]) is None
    assert surface_of(prompt(), [], None) is None


def test_signals_show_what_the_rules_read(claude):
    events = [telemetry(**{"app.entrypoint": "weird", "service.name": "claude-code"})]
    assert signals(prompt(), events, claude) == {"service.name": "claude-code", "app.entrypoint": "weird"}


def test_rules_and_names_are_spec_data():
    spec = parse_spec(
        "t.yaml",
        "id: t\nname: T\ndetect: []\ninstall: {file: x, merge: {}}\n"
        "surfaces: {kiosk: {name: Kiosk, icon: app}}\n"
        "surface: [{when: {path: client, equals: k}, surface: kiosk}]\n",
        {"surfaces": {"cli": {"name": "CLI", "icon": "terminal"}}},
    )
    assert surface_of(prompt({"client": "k"}), [], spec) == "kiosk"
    assert spec.surface_info("kiosk") == {"id": "kiosk", "name": "Kiosk", "icon": "app"}
    assert set(catalog([spec])) == {"cli", "kiosk"}


def test_a_rule_naming_an_unknown_surface_is_refused():
    with pytest.raises(Exception, match="surfaces without a name"):
        parse_spec("t.yaml", "id: t\nname: T\ndetect: []\ninstall: {file: x, merge: {}}\nsurface: [{when: {path: a}, surface: nope}]\n", {})
