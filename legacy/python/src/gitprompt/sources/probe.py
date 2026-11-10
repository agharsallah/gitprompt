"""What GitPrompt reads about a tool on this machine: is it installed, which version, and does
policy (organization or user settings) allow GitPrompt to connect. Read-only."""

import glob
import json
import os
import re
import subprocess
import time
import tomllib
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from gitprompt.core.config import Settings
from gitprompt.core.payload import pluck
from gitprompt.sources.spec import Policy, Spec, expand


def detected(spec: Spec, settings: Settings) -> bool:
    return any(expand(p, settings).exists() for p in spec.detect)


# --- tool version ----------------------------------------------------------------------------


class _TTLCache:
    def __init__(self, max_age: float) -> None:
        self.max_age = max_age
        self._items: dict[Any, tuple[float, Any]] = {}

    def get_or_set(self, key: Any, compute: Callable[[], Any]) -> Any:
        hit = self._items.get(key)
        if hit and time.monotonic() - hit[0] < self.max_age:
            return hit[1]
        value = compute()
        self._items[key] = (time.monotonic(), value)
        return value


_versions = _TTLCache(max_age=300)


def _run_version(cmd: tuple[str, ...]) -> str | None:
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.TimeoutExpired):
        return None
    return version_of(out)


def version_of(output: str) -> str | None:
    """The version in a `--version` output: the first dotted number ("2.1.317 (Claude Code)",
    "GitHub Copilot CLI 1.0.102."), else its first word."""
    if found := re.search(r"\d+(?:\.\d+)+(?:[-+][\w.]+)?", output):
        return found[0].rstrip(".")
    return output.split()[0] if output.strip() else None


def _release(version: str) -> tuple[int, ...]:
    return tuple(int(n) for n in re.findall(r"\d+", version.split("-")[0].split("+")[0]))


def missing_capabilities(spec: Spec, version: str | None) -> dict[str, str]:
    """Capabilities the installed version doesn't have yet (spec `capability_since`), with the
    version that adds each. An unknown version is given the benefit of the doubt."""
    if not version or not _release(version):
        return {}
    return {c: since for c, since in spec.capability_since.items() if _release(version) < _release(since)}


def tool_version(spec: Spec) -> str | None:
    """The tool's `--version`, cached for a few minutes (it is also stamped on every prompt)."""
    cmd = tuple(spec.version_cmd)
    return _versions.get_or_set(cmd, lambda: _run_version(cmd)) if cmd else None


# --- policy ----------------------------------------------------------------------------------


@dataclass
class PolicyReport:
    blocks: list[str] = field(default_factory=list)  # GitPrompt must not connect
    warnings: list[str] = field(default_factory=list)  # it may connect, but something may interfere


@dataclass(frozen=True)
class SettingsFile:
    """One settings file named by `policy.files`, as seen by the rules below."""

    name: str
    doc: dict[str, Any]
    policy: Policy
    ours: re.Pattern[str]  # matches GitPrompt's own receiver URL for this source
    receiver: str


# A rule reads one settings file and adds to the report. To support a new policy key: add a field
# to `Policy` (spec.py) and a rule here.
Rule = Callable[[SettingsFile, PolicyReport], None]


def _matches(value: Any, values: list[Any]) -> bool:
    return value in values or (isinstance(value, list) and any(v in value for v in values))


def _block_if(f: SettingsFile, report: PolicyReport) -> None:
    for path, values in f.policy.block_if.items():
        value = pluck(f.doc, path)
        if value is not None and _matches(value, values):
            report.blocks.append(f"{path} is set in {f.name}")


def _block_if_set(f: SettingsFile, report: PolicyReport) -> None:
    for path in f.policy.block_if_set:
        if (value := pluck(f.doc, path)) and not f.ours.search(str(value)):
            report.blocks.append(f"{path} is already set in {f.name}")


def _warn_if_set(f: SettingsFile, report: PolicyReport) -> None:
    for path in f.policy.warn_if_set:
        if pluck(f.doc, path) is not None:
            report.warnings.append(f"{path} is set in {f.name}: it must allow {f.receiver}")


FILE_RULES: list[Rule] = [_block_if, _block_if_set, _warn_if_set]


# How a settings file is parsed, by suffix; anything else is JSON. Read-only: tomllib can't write.
PARSERS: dict[str, Callable[[str], Any]] = {".toml": tomllib.loads}


def _parse(name: str) -> Any:
    return PARSERS.get(Path(name).suffix.lower(), json.loads)(Path(name).read_text())


def _settings_files(spec: Spec, settings: Settings) -> Iterator[SettingsFile]:
    for pattern in spec.policy.files:
        for name in sorted(glob.glob(str(expand(pattern, settings)))):
            try:
                doc = _parse(name)
            except (OSError, ValueError):  # tomllib.TOMLDecodeError is a ValueError
                continue  # unreadable or not valid: the tool can't apply it either
            if isinstance(doc, dict):
                yield SettingsFile(name, doc, spec.policy, spec.marker, settings.receiver)


def check_policy(spec: Spec, settings: Settings) -> PolicyReport:
    """Blocks and warnings found in settings files and the environment. GitPrompt reports policy;
    it never works around it."""
    report = PolicyReport()
    for settings_file in _settings_files(spec, settings):
        for rule in FILE_RULES:
            rule(settings_file, report)
    report.warnings += [
        f"{var} is set in this shell's environment: where it is also set, it wins over the settings file"
        for var in spec.policy.warn_env
        if var in os.environ
    ]
    return report
