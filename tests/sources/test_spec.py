from gitprompt.sources.spec import BUNDLED, expand, load_specs


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
