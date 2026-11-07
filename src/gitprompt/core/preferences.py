"""User preferences: what the Settings page can change, stored in `~/.gitprompt/preferences.json`.

Each value comes from the first of these that sets it:

    env      GITPROMPT_<KEY> (e.g. GITPROMPT_EVENT_RETENTION_DAYS=30): locked, the page shows it read-only
    file     preferences.json, written by PUT /api/settings (atomic, mode 0600)
    default  the field default in `Preferences`

To add a preference, add a field to `Preferences`. If it only takes effect at start-up, list it in
`APPLIES_AT_START` so the API says a restart is needed. The file and env vars are re-read on every
use, so every other change applies at once.

Where the data lives (GITPROMPT_HOME), the port and the log level stay env-only (core/config.py):
the file itself lives under GITPROMPT_HOME, and a new port would break every connected hook.
"""

import json
import logging
import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError

from gitprompt.core.errors import Conflict, GitPromptError
from gitprompt.core.files import atomic_write

ENV_PREFIX = "GITPROMPT_"
FILE_NAME = "preferences.json"
log = logging.getLogger(__name__)

RangeId = Literal["24h", "7d", "30d", "all"]  # web/src/lib/insights/range.ts RANGES
Source = Literal["env", "file", "default"]


class Preferences(BaseModel):
    model_config = ConfigDict(extra="forbid")

    default_range: RangeId = "7d"  # time window the insight pages open with
    week_start: Literal["monday", "sunday"] = "monday"  # first row of the weekday heatmap
    event_retention_days: int | None = Field(None, ge=1, le=3650)  # None = keep raw events forever
    hide_secrets_on_ingest: bool = False  # replace detected secrets before a payload is stored


APPLIES_AT_START: frozenset[str] = frozenset()  # keys that need a daemon restart to take effect

KEYS = tuple(Preferences.model_fields)


class InvalidPreferences(GitPromptError):
    """A value (from the request, the file or an env var) doesn't fit `Preferences`."""

    status = 422


class Locked(Conflict):
    """The value is set by an environment variable, which wins over the file."""


@dataclass(frozen=True)
class Resolved:
    key: str
    value: Any
    default: Any
    source: Source
    env: str  # the variable that would lock it
    applies: Literal["now", "restart"]

    @property
    def locked(self) -> bool:
        return self.source == "env"


def env_name(key: str) -> str:
    return ENV_PREFIX + key.upper()


def _errors(error: ValidationError, origin: str) -> str:
    return "; ".join(f"{origin} {'.'.join(map(str, e['loc']))}: {e['msg']}" for e in error.errors())


class PreferenceFile:
    """preferences.json under a data directory, plus the env vars that override it."""

    def __init__(self, home: Path, environ: Mapping[str, str] | None = None) -> None:
        self.path = home / FILE_NAME
        self.environ = os.environ if environ is None else environ

    # --- reading ---

    def stored(self) -> dict[str, Any]:
        """The values the file sets (validated). An unreadable or invalid file raises InvalidPreferences."""
        if not self.path.exists():
            return {}
        try:
            data = json.loads(self.path.read_text() or "{}")
        except json.JSONDecodeError as e:
            raise InvalidPreferences(f"{self.path} is not valid JSON ({e.msg}); fix or delete it") from None
        if not isinstance(data, dict):
            raise InvalidPreferences(f"{self.path} must hold a JSON object; fix or delete it")
        try:
            Preferences.model_validate(data)
        except ValidationError as e:
            raise InvalidPreferences(_errors(e, str(self.path))) from None
        return data

    def overrides(self) -> dict[str, str]:
        """Preferences set by a non-empty GITPROMPT_<KEY> env var."""
        return {k: v for k in KEYS if (v := self.environ.get(env_name(k), "").strip())}

    def resolve(self) -> dict[str, Resolved]:
        """Every preference with its effective value and where it came from."""
        stored, env = self.stored(), self.overrides()
        try:
            values = Preferences.model_validate(stored | env)
        except ValidationError as e:
            raise InvalidPreferences(_errors(e, "environment variable")) from None
        defaults = Preferences()
        return {
            key: Resolved(
                key=key,
                value=getattr(values, key),
                default=getattr(defaults, key),
                source="env" if key in env else "file" if key in stored else "default",
                env=env_name(key),
                applies="restart" if key in APPLIES_AT_START else "now",
            )
            for key in KEYS
        }

    def current(self) -> Preferences:
        return Preferences.model_validate({k: r.value for k, r in self.resolve().items()})

    def effective(self) -> Preferences:
        """`current`, or the defaults when the file or an env var is invalid: the receiver and the
        retention pass keep working on safe values while the Settings page reports the error."""
        try:
            return self.current()
        except InvalidPreferences as e:
            log.warning("using default preferences: %s", e)
            return Preferences()

    # --- writing ---

    def update(self, changes: Mapping[str, Any]) -> list[str]:
        """Set values in the file; a value of None removes the key (back to the default).
        Returns the keys whose effective value changed. Nothing is written if any change is refused."""
        unknown = sorted(set(changes) - set(KEYS))
        if unknown:
            raise InvalidPreferences(f"unknown setting(s): {', '.join(unknown)}")
        before = self.resolve()
        if locked := sorted(k for k in changes if before[k].locked):
            names = ", ".join(before[k].env for k in locked)
            raise Locked(f"set by an environment variable ({names}); unset it to change this here")
        stored = self.stored()
        for key, value in changes.items():
            if value is None:
                stored.pop(key, None)
            else:
                stored[key] = value
        try:
            clean = Preferences.model_validate(stored).model_dump(include=set(stored))
        except ValidationError as e:
            raise InvalidPreferences(_errors(e, "setting")) from None
        atomic_write(self.path, json.dumps(clean, indent=2, sort_keys=True) + "\n")
        after = self.resolve()
        return [k for k in KEYS if after[k].value != before[k].value]
