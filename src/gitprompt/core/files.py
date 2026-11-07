"""File writes that never leave a half-written file behind."""

import os
import tempfile
from pathlib import Path


def atomic_write(path: Path, text: str, mode: int = 0o600) -> None:
    """Write `text` to a temporary file next to `path`, then rename it over `path` in one step.
    A crash mid-write leaves the old file intact. The new file gets `mode`."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=path.parent, prefix=f".{path.name}.", delete=False) as tmp:
        tmp.write(text)
        tmp.flush()
        os.fsync(tmp.fileno())
    try:
        os.chmod(tmp.name, mode)
        os.replace(tmp.name, path)
    except BaseException:
        os.unlink(tmp.name)
        raise
