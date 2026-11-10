"""Event retention: while the daemon runs, raw events older than the `event_retention_days`
preference are deleted, at start-up and then every few hours. Prompts and notes are never removed.

The preference is re-read on every pass, so changing it on the Settings page needs no restart.
Without the preference (the default) nothing is ever deleted.
"""

import asyncio
import logging

from gitprompt.capture import compact
from gitprompt.core.preferences import PreferenceFile
from gitprompt.storage.store import Store

EVERY_S = 6 * 3600
log = logging.getLogger(__name__)


def apply(store: Store, preferences: PreferenceFile) -> int:
    """One retention pass. Returns the number of events deleted (0 when retention is off)."""
    days = preferences.effective().event_retention_days
    return compact.remove(store, compact.expired(store, days)) if days else 0


async def keep(store: Store, preferences: PreferenceFile, every_s: float = EVERY_S) -> None:
    while True:
        if removed := apply(store, preferences):
            log.info("retention: deleted %d events", removed)
        await asyncio.sleep(every_s)
