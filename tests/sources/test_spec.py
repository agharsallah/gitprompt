import pytest

from gitprompt.sources import probe
from gitprompt.sources.spec import BUNDLED, expand, load_spec_set, load_specs, matches


def test_layers_name_their_main_connector(settings):
    specs = load_specs(settings)
    assert specs["claude-code"].part_of is None and specs["claude-code"].layer == "Hooks"
    assert specs["claude-code-otel"].part_of == "claude-code" and specs["claude-code-otel"].layer == "Telemetry"


def test_part_of_must_name_a_main_spec(settings):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "mytool-otel.yaml").write_text(MINIMAL.replace("id: mytool", "id: mytool-otel") + "part_of: nowhere\n")
    loaded = load_spec_set(settings)
    assert "mytool-otel" not in loaded.specs and "claude-code-otel" in loaded.specs
    assert [p.file.name for p in loaded.problems] == ["mytool-otel.yaml"] and "part_of" in loaded.problems[0].error


MINIMAL = "id: mytool\nname: My tool\ndetect: [~/.mytool]\ninstall: {file: ~/.mytool/hooks.json, merge: {}}\n"


@pytest.mark.parametrize(
    "text",
    ["id: [unclosed\n", "- a list\n", "id: mytool\n", MINIMAL + "capabilities: [telepathy]\n", MINIMAL + "join: guess\n"],
    ids=["not yaml", "not a mapping", "missing keys", "unknown capability", "unknown join"],
)
def test_a_broken_user_file_is_reported_and_skipped(settings, text):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "mytool.yaml").write_text(text)
    loaded = load_spec_set(settings)
    assert {"claude-code", "claude-code-otel"} <= loaded.specs.keys() and "mytool" not in loaded.specs
    [problem] = loaded.problems
    assert problem.file.name == "mytool.yaml" and problem.file.origin == "user" and "mytool.yaml" in problem.error
    assert problem.fallback is None


def test_a_broken_override_falls_back_to_the_bundled_spec(settings):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "claude-code.yaml").write_text("id: claude-code\nname: [broken\n")
    loaded = load_spec_set(settings)
    assert loaded.specs["claude-code"].name == "Claude Code" and loaded.files["claude-code"].origin == "bundled"
    [problem] = loaded.problems
    assert problem.file.overrides and problem.fallback == "claude-code"


def test_files_say_where_each_spec_came_from(settings):
    settings.user_sources.mkdir(parents=True)
    bundled = (BUNDLED / "claude-code.yaml").read_text()
    (settings.user_sources / "claude-code.yaml").write_text(bundled.replace("name: Claude Code", "name: Patched"))
    (settings.user_sources / "mytool.yaml").write_text(MINIMAL)
    files = load_spec_set(settings).files
    assert files["claude-code"].origin == "user" and files["claude-code"].overrides
    assert files["mytool"].origin == "user" and not files["mytool"].overrides
    assert files["claude-code-otel"].origin == "bundled" and files["claude-code-otel"].path.endswith("claude-code-otel.yaml")


def test_a_duplicate_id_keeps_the_first_file(settings):
    settings.user_sources.mkdir(parents=True)
    (settings.user_sources / "a.yaml").write_text(MINIMAL)
    (settings.user_sources / "b.yaml").write_text(MINIMAL.replace("My tool", "Copy"))
    loaded = load_spec_set(settings)
    assert loaded.specs["mytool"].name == "My tool" and "already used by a.yaml" in loaded.problems[0].error


def test_every_bundled_spec_declares_what_it_yields(settings):
    assert all(spec.capabilities for spec in load_specs(settings).values())


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


@pytest.mark.parametrize(
    ("output", "version"),
    [
        ("2.1.317 (Claude Code)\n", "2.1.317"),
        ("GitHub Copilot CLI 1.0.102.\nRun 'copilot update'.\n", "1.0.102"),
        ("nightly\n", "nightly"),
        ("", None),
    ],
)
def test_tool_version_is_the_first_dotted_number(output, version):
    assert probe.version_of(output) == version
