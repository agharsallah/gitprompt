"""Add or remove GitPrompt's entries in a tool's JSON config file, safely.

- Entries inside lists (hook handlers) are recognised by the receiver URL they contain.
- Plain values we add (e.g. env vars) are recorded at install time and removed only if unchanged,
  so values the user had before, or edited since, are never touched.
- The file is never written if it can't be parsed; a timestamped backup is taken before each write;
  writes are atomic and keep the file mode.
"""

import difflib
import json
import os
import re
import shutil
import tempfile
import time
from pathlib import Path
from typing import Any

from gitprompt.core.errors import ConfigError

__all__ = ["ConfigError", "Record", "added", "diff", "dump", "merge", "read", "render", "strip", "uninstall", "write"]

Record = list[tuple[list[str], Any]]  # (key path, value) pairs we added


def render(template: Any, values: dict[str, str]) -> Any:
    """Replace `{{name}}` placeholders in every string of a JSON template."""
    if isinstance(template, dict):
        return {k: render(v, values) for k, v in template.items()}
    if isinstance(template, list):
        return [render(v, values) for v in template]
    if isinstance(template, str):
        for key, value in values.items():
            template = template.replace(f"{{{{{key}}}}}", value)
    return template


def merge(doc: Any, patch: Any) -> Any:
    """Dicts merge recursively, lists get missing items appended, existing scalars are kept."""
    if isinstance(doc, dict) and isinstance(patch, dict):
        return doc | {k: merge(doc[k], v) if k in doc else v for k, v in patch.items()}
    if isinstance(doc, list) and isinstance(patch, list):
        return doc + [item for item in patch if item not in doc]
    return doc


def added(doc: Any, patch: Any, path: tuple[str, ...] = ()) -> Record:
    """Plain values that merging `patch` adds to `doc` (lists are handled by `strip`)."""
    if isinstance(patch, dict):
        return [x for k, v in patch.items() for x in added(doc.get(k) if isinstance(doc, dict) else None, v, (*path, k))]
    return [] if doc is not None or isinstance(patch, list) else [(list(path), patch)]


def strip(node: Any, ours: re.Pattern[str]) -> Any:
    """Remove entries that hold our URL in one of their own string values, and any container
    that becomes empty only because of that removal. Everything else is left untouched."""
    if isinstance(node, list):
        out = []
        for item in node:
            if isinstance(item, dict) and any(isinstance(v, str) and ours.search(v) for v in item.values()):
                continue
            new = strip(item, ours)
            if new or new == item:
                out.append(new)
        return out
    if isinstance(node, dict):
        return {k: new for k, v in node.items() if not (isinstance(v, str) and ours.search(v)) and ((new := strip(v, ours)) or new == v)}
    return node


def uninstall(doc: dict[str, Any], ours: re.Pattern[str], record: Record) -> dict[str, Any]:
    """Remove our list entries (by URL) and the plain values in `record` that are still unchanged."""
    doc = strip(doc, ours)  # returns fresh containers, safe to edit below
    for path, value in record:
        parents, node = [], doc
        for key in path[:-1]:
            parents.append((node, key))
            node = node.get(key) if isinstance(node, dict) else None
        if not isinstance(node, dict) or node.get(path[-1], object()) != value:
            continue  # changed by the user since: keep it
        del node[path[-1]]
        for parent, key in reversed(parents):
            if parent[key]:
                break
            del parent[key]
    return doc


def read(path: Path) -> tuple[str, dict[str, Any]]:
    """(text, parsed object) of a JSON config file; ("", {}) if it doesn't exist yet."""
    if not path.exists():
        return "", {}
    text = path.read_text()
    try:
        doc = json.loads(text) if text.strip() else {}
    except ValueError as e:
        raise ConfigError(f"{path} is not valid JSON ({e}); fix it first, GitPrompt won't overwrite it") from e
    if not isinstance(doc, dict):
        raise ConfigError(f"{path} does not hold a JSON object")
    return text, doc


def dump(doc: dict[str, Any]) -> str:
    return json.dumps(doc, indent=2, ensure_ascii=False) + "\n"


def diff(path: Path, before: str, after: str) -> str:
    lines = difflib.unified_diff(before.splitlines(True), after.splitlines(True), str(path), str(path))
    return "".join(lines)


def write(path: Path, text: str, backups: Path) -> Path | None:
    """Back up the current file (if any), then replace it atomically keeping its mode. Returns the backup."""
    backup = None
    mode = path.stat().st_mode & 0o777 if path.exists() else 0o600
    if path.exists():
        backups.mkdir(mode=0o700, parents=True, exist_ok=True)
        backup = backups / f"{path.name}.{time.strftime('%Y%m%d-%H%M%S')}"
        shutil.copy2(path, backup)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=path.parent, delete=False) as tmp:
        tmp.write(text)
    os.chmod(tmp.name, mode)
    os.replace(tmp.name, path)
    return backup
