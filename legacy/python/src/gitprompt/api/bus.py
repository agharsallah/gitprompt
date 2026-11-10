"""In-process fan-out of "something was captured" notices to every open dashboard stream."""

import asyncio
import contextlib
from collections.abc import AsyncIterator
from typing import Any

Notice = dict[str, Any]


class Bus:
    def __init__(self, backlog: int = 100) -> None:
        self._backlog = backlog
        self._queues: set[asyncio.Queue[Notice]] = set()

    def publish(self, notice: Notice) -> None:
        for queue in self._queues:
            # A notice only means "reload": dropping one for a stalled subscriber loses nothing.
            with contextlib.suppress(asyncio.QueueFull):
                queue.put_nowait(notice)

    @contextlib.asynccontextmanager
    async def subscribe(self) -> AsyncIterator[asyncio.Queue[Notice]]:
        queue: asyncio.Queue[Notice] = asyncio.Queue(self._backlog)
        self._queues.add(queue)
        try:
            yield queue
        finally:
            self._queues.discard(queue)
