import pytest

from gitprompt.core.errors import SpecError
from gitprompt.sources.spec import BUNDLED, expand, load_specs, matches


def test_layers_name_their_main_connector(settings):
    specs = load_specs(settings)
    assert specs["claude-code"].part_of is None and specs["claude-code"].layer == "Hooks"
    assert specs["claude-code-otel"].part_of == "claude-code" and specs["claude-code-otel"].layer == "Telemetry"


def test_part_of_must_name_a_main_spec(settings):
    settings.user_sources.mkdir(parents=True)
    bundled = (BUNDLED / "claude-code-otel.yaml").read_text()
    (settings.user_sources / "claude-code-otel.yaml").write_text(bundled.replace("part_of: claude-code", "part_of: nowhere"))
    with pytest.raises(SpecError, match="part_of"):
        load_specs(settings)


def test_expand_uses_env_default_and_user_home(settings, monkeypatch):
    assert expand("${CLAUDE_CONFIG_DIR:-~/.claude}/settings.json", settings) == settings.user_home / ".claude/settings.json"
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", "/elsewhere")
    assert str(expand("${CLAUDE_CONFIG_DIR:-~/.claude}/settings.json", settings)) == "/elsewhere/settings.json"


def test_bundled_spec_inherits_default_map(settings):
    spec = load_specs(settings)["claude-code"]
    assert spec.map["prompt"][0] == "prompt"  # from _defaults.yaml
    assert spec.map["permission_mode"] == ["permission_mode"]  # from the spec


def test_user_spec_overrides_bundled(settings):
    settings.user_sources.mkdir(parents=True)
    bundled = (BUNDLED / "claude-code.yaml").read_text()
    (settings.user_sources / "claude-code.yaml").write_text(bundled.replace("name: Claude Code", "name: Patched"))
    assert load_specs(settings)["claude-code"].name == "Patched"


def test_drop_events_match_exact_names_and_trailing_globs(settings):
    spec = load_specs(settings)["claude-code-otel"]
    assert not spec.keeps("hook_registered") and not spec.keeps("hook_execution_complete")
    assert spec.keeps("api_request") and spec.keeps("hook_registered_later") and spec.keeps(None)
    assert matches("hook_*", "hook_x") and not matches("hook_*", "a_hook_x") and not matches("hook", "hook_x")


def test_accept_events_apply_before_drop_events(settings):
    spec = load_specs(settings)["claude-code"].model_copy(update={"accept_events": ["Stop", "Notification"], "drop_events": {"Sto*": "no"}})
    assert spec.keeps("Notification") and not spec.keeps("Stop") and not spec.keeps("PostToolUse")
