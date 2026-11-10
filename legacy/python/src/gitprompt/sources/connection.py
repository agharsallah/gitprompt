"""Connecting a source = merging its spec's `install.merge` into the tool's config file, with
consent: every change can be previewed (dry run) as the exact diff before it is written.

This module holds the workflow; installer.py holds the JSON-file mechanics it is built on.
"""

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from gitprompt.core.config import Settings
from gitprompt.core.errors import ConfigError, PolicyBlocked
from gitprompt.sources import installer, probe
from gitprompt.sources.spec import Spec, expand


@dataclass
class Change:
    file: str
    diff: str  # unified diff; empty when there is nothing to change
    applied: bool
    backup: str | None = None


class Connection:
    """GitPrompt's entries for one source in one tool config file."""

    def __init__(self, spec: Spec, settings: Settings, token: str | None = None) -> None:
        self.spec, self.settings = spec, settings
        self.token = token  # render with this token instead of the current one (token rotation preview)
        self.config_file = expand(spec.install.file, settings)
        # Plain values we added at connect time, so disconnect removes exactly those.
        self.record_file = settings.home / "installed" / f"{spec.id}.json"

    # --- state ---

    def patch(self) -> dict[str, Any]:
        values = {"receiver": self.settings.receiver, "source": self.spec.id, "token": self.token or self.settings.token()}
        return installer.render(self.spec.install.merge, values)

    @property
    def connected(self) -> bool:
        return self.config_file.exists() and bool(self.spec.marker.search(self.config_file.read_text()))

    @property
    def outdated(self) -> bool:
        """Connected, but the spec now adds more (or other) entries than what is installed."""
        if not self.connected:
            return False
        try:
            return bool(self._change(self.patch(), dry_run=True).diff)
        except ConfigError:
            return False

    # --- actions ---

    def connect(self, dry_run: bool = False) -> Change:
        if blocks := probe.check_policy(self.spec, self.settings).blocks:
            raise PolicyBlocked("Blocked by settings GitPrompt won't override: " + "; ".join(blocks))
        return self._change(self.patch(), dry_run)

    def disconnect(self, dry_run: bool = False) -> Change:
        return self._change(None, dry_run)

    # --- internals ---

    def _read_record(self) -> installer.Record:
        return json.loads(self.record_file.read_text()) if self.record_file.exists() else []

    def _write_record(self, record: installer.Record) -> None:
        self.record_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.record_file.write_text(json.dumps(record))

    def _change(self, patch: dict[str, Any] | None, dry_run: bool) -> Change:
        """Remove our previous entries, then add `patch` (if any): reconnecting never duplicates."""
        path = self.config_file
        before, doc = installer.read(path)
        clean = installer.uninstall(doc, self.spec.marker, self._read_record())
        new = installer.merge(clean, patch) if patch else clean
        if new == doc:
            return Change(file=str(path), diff="", applied=False)
        after = installer.dump(new)
        change = Change(file=str(path), diff=installer.diff(path, before, after), applied=not dry_run)
        if not dry_run:
            backup = installer.write(path, after, self.settings.backups)
            change.backup = _str(backup)
            self._write_record(installer.added(clean, patch) if patch else [])
        return change


def _str(path: Path | None) -> str | None:
    return str(path) if path else None
