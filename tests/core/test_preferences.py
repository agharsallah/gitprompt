"""Preferences: env > file > defaults, validation, atomic writes."""

import json

import pytest

from gitprompt.core.preferences import InvalidPreferences, Locked, PreferenceFile


@pytest.fixture
def prefs(tmp_path):
    return PreferenceFile(tmp_path, environ={})


def test_defaults_without_a_file(prefs):
    resolved = prefs.resolve()
    assert resolved["default_range"].value == "7d" and resolved["default_range"].source == "default"
    assert prefs.current().event_retention_days is None
    assert not prefs.path.exists()


def test_env_wins_over_file_wins_over_default(tmp_path):
    (tmp_path / "preferences.json").write_text(json.dumps({"default_range": "30d", "week_start": "sunday"}))
    prefs = PreferenceFile(tmp_path, environ={"GITPROMPT_DEFAULT_RANGE": "24h", "GITPROMPT_EVENT_RETENTION_DAYS": "14"})
    r = prefs.resolve()
    assert (r["default_range"].value, r["default_range"].source, r["default_range"].locked) == ("24h", "env", True)
    assert (r["week_start"].value, r["week_start"].source) == ("sunday", "file")
    assert (r["event_retention_days"].value, r["event_retention_days"].source) == (14, "env")  # parsed from text
    assert (r["hide_secrets_on_ingest"].value, r["hide_secrets_on_ingest"].source) == (False, "default")


def test_empty_env_var_does_not_lock(tmp_path):
    prefs = PreferenceFile(tmp_path, environ={"GITPROMPT_WEEK_START": " "})
    assert prefs.resolve()["week_start"].source == "default"


def test_update_writes_the_file_and_reports_changes(prefs):
    assert prefs.update({"week_start": "sunday", "event_retention_days": 30}) == ["week_start", "event_retention_days"]
    assert json.loads(prefs.path.read_text()) == {"event_retention_days": 30, "week_start": "sunday"}
    assert prefs.path.stat().st_mode & 0o777 == 0o600
    assert prefs.update({"week_start": "sunday"}) == []  # same value: nothing changed
    assert prefs.update({"event_retention_days": None}) == ["event_retention_days"]  # null resets to default
    assert json.loads(prefs.path.read_text()) == {"week_start": "sunday"}


@pytest.mark.parametrize(
    "changes",
    [{"default_range": "1y"}, {"event_retention_days": 0}, {"week_start": 3}, {"nope": 1}],
)
def test_invalid_values_are_refused_and_nothing_is_written(prefs, changes):
    prefs.update({"default_range": "30d"})
    before = prefs.path.read_text()
    with pytest.raises(InvalidPreferences):
        prefs.update(changes)
    assert prefs.path.read_text() == before


def test_env_locked_values_cannot_be_changed(tmp_path):
    prefs = PreferenceFile(tmp_path, environ={"GITPROMPT_EVENT_RETENTION_DAYS": "7"})
    with pytest.raises(Locked, match="GITPROMPT_EVENT_RETENTION_DAYS"):
        prefs.update({"event_retention_days": 30, "week_start": "sunday"})
    assert not prefs.path.exists()  # refused as a whole


def test_invalid_file_is_reported_and_effective_falls_back_to_defaults(prefs):
    prefs.path.write_text("{oops")
    with pytest.raises(InvalidPreferences, match="not valid JSON"):
        prefs.resolve()
    with pytest.raises(InvalidPreferences):
        prefs.update({"week_start": "sunday"})  # never overwrite a file the user has to fix
    assert prefs.path.read_text() == "{oops"
    assert prefs.effective().default_range == "7d"


def test_invalid_env_value_is_reported(tmp_path):
    prefs = PreferenceFile(tmp_path, environ={"GITPROMPT_EVENT_RETENTION_DAYS": "forever"})
    with pytest.raises(InvalidPreferences, match="environment variable"):
        prefs.resolve()


def test_write_is_atomic(prefs, monkeypatch):
    prefs.update({"week_start": "sunday"})
    before = prefs.path.read_text()

    def crash(*_):
        raise OSError("disk full")

    monkeypatch.setattr("gitprompt.core.files.os.replace", crash)
    with pytest.raises(OSError):
        prefs.update({"default_range": "30d"})
    assert prefs.path.read_text() == before  # the old file is intact
    assert [p.name for p in prefs.path.parent.iterdir()] == ["preferences.json"]  # no temp file left
