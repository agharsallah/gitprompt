"""Receivers for hook calls and OpenTelemetry exports. Both decode the body by the source's
`format` and run the same capture pipeline (capture/pipeline.py); only the reply differs."""

from typing import Annotated, Any, Literal

from fastapi import APIRouter, Body, Depends

from gitprompt.api.deps import BusDep, SpecDep, StoreDep, require_token
from gitprompt.capture.pipeline import capture
from gitprompt.core.errors import NotFound

router = APIRouter(tags=["ingest"], dependencies=[Depends(require_token)])
Payload = Annotated[dict[str, Any], Body()]


@router.post("/ingest/{source}", status_code=204)
async def ingest(spec: SpecDep, payload: Payload, store: StoreDep, bus: BusDep) -> None:
    """Hook endpoint. Always an empty 2xx, which Claude Code treats as "no output": nothing is
    blocked, changed or added to the conversation."""
    if captured := await capture(spec, store, payload):
        bus.publish(captured.notice())


@router.post("/ingest/{source}/v1/{signal}")
async def ingest_otlp(spec: SpecDep, signal: Literal["logs", "metrics"], payload: Payload, store: StoreDep, bus: BusDep) -> dict:
    """OTLP/HTTP JSON endpoint: the exporter appends /v1/logs and /v1/metrics to the configured URL."""
    if spec.format != "otlp":
        raise NotFound(f"source {spec.id!r} does not receive OpenTelemetry exports")
    if captured := await capture(spec, store, payload):
        bus.publish(captured.notice())
    return {}  # an empty ExportServiceResponse: everything accepted
