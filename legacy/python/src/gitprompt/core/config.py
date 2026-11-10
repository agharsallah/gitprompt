"""Runtime settings: where GitPrompt keeps its data and how the receiver is reached.

Each setting can be changed with an environment variable:

GITPROMPT_HOME       data directory (db, token, backups, user specs)   default ~/.gitprompt
GITPROMPT_PORT       port the receiver and dashboard listen on         default 7878
GITPROMPT_LOG_LEVEL  uvicorn log level                                 default warning

The host is fixed to loopback on purpose: the prompt API has no auth beyond the Host check,
so binding elsewhere would expose every stored prompt to the network.
"""

import os
import secrets
from dataclasses import dataclass, field
from pathlib import Path


def _env_int(name: str, default: int) -> int:
    value = os.environ.get(name)
    if not value:
        return default
    try:
        return int(value)
    except ValueError:
        raise ValueError(f"{name} must be an integer, got {value!r}") from None


@dataclass(frozen=True)
class Settings:
    home: Path = field(default_factory=lambda: Path(os.environ.get("GITPROMPT_HOME", "~/.gitprompt")).expanduser())
    user_home: Path = field(default_factory=Path.home)  # base for "~" in source specs (overridden in tests)
    host: str = "127.0.0.1"
    port: int = field(default_factory=lambda: _env_int("GITPROMPT_PORT", 7878))
    log_level: str = field(default_factory=lambda: os.environ.get("GITPROMPT_LOG_LEVEL", "warning"))

    @property
    def receiver(self) -> str:
        return f"http://{self.host}:{self.port}"

    @property
    def db_path(self) -> Path:
        return self.home / "gitprompt.db"

    @property
    def user_sources(self) -> Path:
        """Drop a YAML spec here to add or fix a source without a new release."""
        return self.home / "sources"

    @property
    def backups(self) -> Path:
        return self.home / "backups"

    @property
    def token_file(self) -> Path:
        return self.home / "token"

    def token(self) -> str:
        """Shared secret that hook calls must present. Created once, readable by the user only."""
        path = self.token_file
        if not path.exists():
            self.home.mkdir(mode=0o700, parents=True, exist_ok=True)
            path.write_text(secrets.token_urlsafe(32))
            path.chmod(0o600)
        return path.read_text().strip()
