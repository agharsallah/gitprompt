"""Receivers for hook calls and OpenTelemetry exports. Both decode the body by the source's
`format` and run the same capture pipeline (capture/pipeline.py); only the reply differs. They do
not derive anything: the turns that got events are marked, and the prompt index catches up on the
next read (insights/indexer.py), so a long turn costs one recomputation per read, not per event."""

from typing import Annotated, Any, Literal

from fastapi import APIRouter, Body, Depends

from gitprompt.api.deps import BusDep, PreferencesDep, SpecDep, StoreDep, require_token
from gitprompt.capture.pipeline import Captured, capture
from gitprompt.core.errors import NotFound
from gitprompt.core.preferences import PreferenceFile
from gitprompt.sources.spec import Spec
from gitprompt.storage.store import Store

router = APIRouter(tags=["ingest"], dependencies=[Depends(require_token)])
Payload = Annotated[dict[str, Any], Body()]


@router.post("/ingest/{source}", status_code=204)
async def ingest(spec: SpecDep, payload: Payload, store: StoreDep, bus: BusDep, preferences: PreferencesDep) -> None:
    """Hook endpoint. Always an empty 2xx, which Claude Code treats as "no output": nothing is
    blocked, changed or added to the conversation."""
    if captured := await _capture(spec, store, payload, preferences):
        store.index.mark(captured.turns)  # recomputed on the next read (insights/indexer.py)
        bus.publish(captured.notice())


@router.post("/ingest/{source}/v1/{signal}")
async def ingest_otlp(
    spec: SpecDep, signal: Literal["logs", "metrics"], payload: Payload, store: StoreDep, bus: BusDep, preferences: PreferencesDep
) -> dict:
    """OTLP/HTTP JSON endpoint: the exporter appends /v1/logs and /v1/metrics to the configured URL."""
    if spec.format != "otlp":
        raise NotFound(f"source {spec.id!r} does not receive OpenTelemetry exports")
    if captured := await _capture(spec, store, payload, preferences):
        store.index.mark(captured.turns)  # recomputed on the next read (insights/indexer.py)
        bus.publish(captured.notice())
    return {}  # an empty ExportServiceResponse: everything accepted


async def _capture(spec: Spec, store: Store, payload: dict[str, Any], preferences: PreferenceFile) -> Captured:
    """The pipeline, unless the connector is turned off in GitPrompt: then the payload is discarded
    (still with the same empty answer, so the tool is never affected)."""
    prefs = preferences.effective()
    if spec.id in prefs.disabled_sources:
        return Captured()
    return await capture(spec, store, payload, prefs.hide_secrets_on_ingest)
