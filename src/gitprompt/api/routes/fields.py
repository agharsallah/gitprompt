from fastapi import APIRouter

from gitprompt.api.deps import SpecsDep, StoreDep
from gitprompt.insights.catalog import Catalog, build_catalog

router = APIRouter(prefix="/api/fields", tags=["fields"])


@router.get("")
async def catalog(store: StoreDep, specs: SpecsDep) -> Catalog:
    """Every field seen in stored payloads, next to the fields each tool documents."""
    return build_catalog(store.samples(), specs.values())
