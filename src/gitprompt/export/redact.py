"""The one redaction pass every export goes through (ADR-0018), before any format renders it.

    redact(data, options, home)   the same JSON with what the options leave out removed or masked

Secrets are always hidden (insights/redaction.SECRETS). The other rules follow `Options`, whose
defaults are the most private choice: no prompt text, repository-relative paths and `~` for the
home folder, no account identity, no raw payloads. A format never redacts on its own: it only sees
what this pass returns.
"""

import re
from dataclasses import asdict, dataclass, fields
from pathlib import PurePosixPath
from typing import Any

from gitprompt.insights.redaction import hide

# Keys that hold text you typed, or text written from it: system excerpts, subagent briefs, session
# titles, search words. Dropped unless `prompt_text`.
TEXT_KEYS = frozenset({"text", "typed_text", "prompt", "excerpt", "label", "title", "snippet", "diff", "q"})
# Stored hook or telemetry payloads, as received. Dropped unless `drop_raw` is off.
RAW_KEYS = frozenset({"raw", "fields", "context", "events"})
# Who you are: telemetry resource attributes of the account, and commit authors.
IDENTITY_PREFIXES = ("user.", "organization.")
IDENTITY_KEYS = frozenset({"author", "email", "user_email", "user_id", "account_id", "account_uuid", "organization_id"})
# Keys whose value is a project folder (a git root, or the working directory outside a repository).
ROOT_KEYS = frozenset({"repo", "git_root"})

EMAIL = re.compile(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+")
# The cut end of a clipped text: a token cut in half could be the start of a secret no pattern matches.
CUT = re.compile(r"[^\s]{6,}…$")
# Someone's home folder in an absolute path (macOS, Linux), whoever's it is.
HOME = re.compile(r"(?<![\w~.-])/(?:Users|home)/[^/\s\"'<>|]+")
# ...and a home folder written as one path component, as tools name per-project folders
# (`~/.claude/projects/-Users-sam-code-app`): the whole component goes.
ENCODED_HOME = re.compile(r"(?<![\w])-(?:Users|home)-[^/\s\"'<>|]+")


@dataclass(frozen=True)
class Options:
    """What an export includes. Every default is the most private choice."""

    prompt_text: bool = False  # what you typed (and system text, subagent briefs, session titles); else only figures
    relative_paths: bool = True  # project folders become repository-relative paths, the home folder `~`
    drop_identity: bool = True  # account email, user and organization ids, commit authors; emails anywhere
    drop_raw: bool = True  # stored hook and telemetry payloads

    def as_dict(self) -> dict[str, bool]:
        return asdict(self)

    @classmethod
    def names(cls) -> list[str]:
        return [f.name for f in fields(cls)]


def roots_in(value: Any) -> set[str]:
    """Project folders named anywhere in a JSON value (under ROOT_KEYS)."""
    found: set[str] = set()
    if isinstance(value, dict):
        for k, v in value.items():
            if k in ROOT_KEYS and isinstance(v, str) and v.startswith("/"):
                found.add(v.rstrip("/"))
            else:
                found |= roots_in(v)
    elif isinstance(value, list | tuple):
        for v in value:
            found |= roots_in(v)
    return found


class _Paths:
    """Absolute paths made relative: inside a project folder, relative to it (the folder itself by
    its name); in a home folder, from `~`."""

    def __init__(self, roots: set[str], home: str | None):
        self.rules: list[tuple[re.Pattern[str], str]] = []
        for root in sorted((r for r in roots if r and r != "/"), key=len, reverse=True):
            escaped = re.escape(root)
            self.rules.append((re.compile(escaped + r"/(?=[^\s])"), ""))
            self.rules.append((re.compile(escaped + r"(?![\w.-])"), PurePosixPath(root).name or root))
        if home and home != "/":
            self.rules.append((re.compile(re.escape(home.rstrip("/")) + r"(?![\w.-])"), "~"))
        self.rules += [(HOME, "~"), (ENCODED_HOME, "…")]

    def __call__(self, text: str) -> str:
        for pattern, replacement in self.rules:
            text = pattern.sub(replacement, text)
        return text


def redact(data: Any, options: Options, home: str | None = None, roots: set[str] | None = None) -> Any:
    """`data` with what `options` leave out dropped (keys) or masked (inside strings). Secrets are
    always hidden. Project folders are found in `data` itself (ROOT_KEYS), plus `roots`."""
    paths = _Paths(roots_in(data) | (roots or set()), home) if options.relative_paths else None

    def text(value: str) -> str:
        value = CUT.sub("…", hide(value))
        if options.drop_identity:
            value = EMAIL.sub("[hidden email]", value)
        return paths(value) if paths else value

    def dropped(key: str) -> bool:
        return (
            (not options.prompt_text and key in TEXT_KEYS)
            or (options.drop_raw and key in RAW_KEYS)
            or (options.drop_identity and (key in IDENTITY_KEYS or key.startswith(IDENTITY_PREFIXES)))
        )

    def walk(value: Any) -> Any:
        if isinstance(value, str):
            return text(value)
        if isinstance(value, dict):
            return {text(k): walk(v) for k, v in value.items() if not dropped(k)}
        if isinstance(value, list | tuple):
            return [walk(v) for v in value]
        return value

    return walk(data)
