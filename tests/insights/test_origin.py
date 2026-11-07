"""Origin and typed text, from the bundled specs' rules, on synthetic prompts."""

import pytest
from pydantic import ValidationError

from gitprompt.insights.origin import classify, split_context
from gitprompt.sources.spec import OriginRule, load_specs

TAGS = ["ide_selection", "system-reminder"]


@pytest.fixture
def spec(settings):
    return load_specs(settings)["claude-code"]


def prompt(text, **raw):
    return {"prompt": text, "fields": {k: v for k, v in raw.items() if k in ("agent_id", "agent_type")}, "raw": {"prompt": text, **raw}}


def event(name, **raw):
    return {"event": name, "raw": raw}


@pytest.mark.parametrize(
    ("text", "raw", "events", "expected"),
    [
        ("Rename the parser module", {}, [], "typed"),
        ("/usr/local is missing from PATH", {}, [], "typed"),  # a path, not a command
        ("/review the last commit", {}, [], "slash_command"),
        ("/compact", {}, [], "slash_command"),
        ("deploy staging", {}, [event("UserPromptExpansion", command_name="deploy")], "slash_command"),
        ("deploy staging", {}, [event("user_prompt", command_name="deploy", **{"app.entrypoint": "cli"})], "slash_command"),
        ("<task-notification><task-id>t1</task-id></task-notification>", {}, [], "task_notification"),
        ('<agent-message from="worker">done</agent-message>', {}, [], "agent_message"),
        ("Summarise the diff", {"agent_id": "a1"}, [], "subagent"),
        ("Summarise the diff", {"agent_type": "Explore"}, [], "subagent"),
        ("Grade this answer", {}, [event("user_prompt", **{"app.entrypoint": "sdk-cli"})], "programmatic"),
        ("Grade this answer", {}, [event("user_prompt", **{"app.entrypoint": "claude-vscode"})], "typed"),
        ("Grade this answer", {}, [event("api_request", **{"app.entrypoint": "sdk-cli"})], "typed"),  # only user_prompt counts
        ("<ide_selection>x = 1</ide_selection>", {}, [], "unknown"),  # nothing typed
    ],
)
def test_origin_of_each_kind_of_prompt(spec, text, raw, events, expected):
    assert classify(prompt(text, **raw), events, spec)["origin"] == expected


def test_injected_context_is_removed_before_text_rules(spec):
    out = classify(prompt("<system-reminder>be brief</system-reminder>\n<task-notification>x</task-notification>"), [], spec)
    assert out["origin"] == "task_notification" and out["typed_text"] == "<task-notification>x</task-notification>"


def test_unknown_without_text_or_spec(spec):
    assert classify({"prompt": None, "fields": {}, "raw": {}}, [], spec) == {"origin": "unknown", "typed_text": None, "injected": []}
    assert classify(prompt("hello"), [], None)["origin"] == "unknown"


@pytest.mark.parametrize(
    ("text", "typed", "kinds"),
    [
        ("Fix the bug", "Fix the bug", []),
        ('<ide_selection path="a.py">x = 1</ide_selection>\nFix the bug', "Fix the bug", ["ide_selection"]),
        ("Fix the bug\n<system-reminder>be brief</system-reminder>", "Fix the bug", ["system-reminder"]),
        (
            "<ide_selection>a</ide_selection><system-reminder>b</system-reminder> Fix <system-reminder>c</system-reminder>",
            "Fix",
            ["ide_selection", "system-reminder", "system-reminder"],
        ),
        ("<ide_selection>only</ide_selection>", "", ["ide_selection"]),
        ("Compare <ide_selection>a</ide_selection> with b", "Compare <ide_selection>a</ide_selection> with b", []),  # middle stays
        ("<ide_selection>unclosed\nFix the bug", "<ide_selection>unclosed\nFix the bug", []),
        ("<pasted_content>x</pasted_content> Fix", "<pasted_content>x</pasted_content> Fix", []),  # not a listed tag
    ],
)
def test_split_context(text, typed, kinds):
    out, injected = split_context(text, TAGS)
    assert out == typed and [i["kind"] for i in injected] == kinds


def test_injected_reports_block_size():
    _, injected = split_context("<system-reminder>abc</system-reminder> hi", TAGS)
    assert injected == [{"kind": "system-reminder", "chars": len("<system-reminder>abc</system-reminder>")}]


def test_path_rule_with_and_without_equals(spec):
    rules = [
        OriginRule(when={"path": "mode", "equals": "batch"}, origin="programmatic"),
        OriginRule(when={"path": "via"}, origin="agent_message"),
    ]
    custom = spec.model_copy(update={"origin": rules})
    assert classify(prompt("hi", mode="batch"), [], custom)["origin"] == "programmatic"
    assert classify(prompt("hi", mode="chat"), [], custom)["origin"] == "typed"
    assert classify(prompt("hi", via="bus"), [], custom)["origin"] == "agent_message"


@pytest.mark.parametrize("when", [{}, {"equals": "x"}, {"text_regex": "("}, {"text_prefix": "x", "colour": "red"}])
def test_invalid_rules_are_rejected(when):
    with pytest.raises(ValidationError):
        OriginRule(when=when, origin="typed")


def test_spec_rules_come_before_shared_ones(settings):
    spec = load_specs(settings)["claude-code"]
    origins = [r.origin for r in spec.origin]
    assert origins[0] == "programmatic" and origins[-1] == "slash_command"  # the shared slash rule is last
    assert "system-reminder" in spec.injected and "ide_selection" in spec.injected
