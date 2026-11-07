"""Rotating the receiver token: a new secret in `~/.gitprompt/token`, then every connected source is
reconnected so its hooks carry the new one.

Same consent rule as Connect: a dry run returns, per connected source, the exact diff that would be
written (the new token shown as a placeholder, it doesn't exist yet). Rotation is refused when a
connected source can't be reconnected (blocked by policy, unreadable config), because that source
would silently stop being captured.
"""

import secrets
from collections.abc import Mapping
from dataclasses import dataclass, field

from gitprompt.core.config import Settings
from gitprompt.core.errors import PolicyBlocked
from gitprompt.core.files import atomic_write
from gitprompt.sources import probe
from gitprompt.sources.connection import Change, Connection
from gitprompt.sources.spec import Spec

PLACEHOLDER = "<new token>"


@dataclass
class Reconnect:
    source: str
    name: str
    change: Change


@dataclass
class Rotation:
    reconnects: list[Reconnect] = field(default_factory=list)
    blocked: list[str] = field(default_factory=list)  # connected sources that could not be reconnected
    applied: bool = False


def rotate(specs: Mapping[str, Spec], settings: Settings, dry_run: bool = True) -> Rotation:
    connected = [s for s in specs.values() if Connection(s, settings).connected]
    blocked = [f"{s.name}: {'; '.join(b)}" for s in connected if (b := probe.check_policy(s, settings).blocks)]
    if blocked:
        if not dry_run:
            raise PolicyBlocked("Can't reconnect " + "; ".join(blocked) + ". Disconnect it first, then rotate.")
        return Rotation(blocked=blocked)
    # Preview first, also before applying: an unreadable config raises here, before the token changes.
    preview = [Reconnect(s.id, s.name, Connection(s, settings, token=PLACEHOLDER).connect(dry_run=True)) for s in connected]
    if dry_run:
        return Rotation(preview)
    atomic_write(settings.token_file, secrets.token_urlsafe(32))
    return Rotation([Reconnect(s.id, s.name, Connection(s, settings).connect()) for s in connected], applied=True)
