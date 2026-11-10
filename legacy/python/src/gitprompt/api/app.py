import asyncio
import os
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as package_version
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.staticfiles import StaticFiles
from starlette.exceptions import HTTPException as StarletteHTTPException
from starlette.middleware.trustedhost import TrustedHostMiddleware
from starlette.responses import Response
from starlette.types import Scope

from gitprompt.api.bus import Bus
from gitprompt.api.routes import ROUTERS
from gitprompt.capture import retention
from gitprompt.core.config import Settings
from gitprompt.core.errors import GitPromptError
from gitprompt.core.preferences import PreferenceFile
from gitprompt.insights.indexer import Indexer
from gitprompt.sources.spec import load_specs
from gitprompt.storage.store import Store

# The dashboard build the Rust daemon also serves (`pnpm --dir web build` -> web/build); GITPROMPT_DASHBOARD overrides it.
DASHBOARD = Path(os.environ.get("GITPROMPT_DASHBOARD") or Path(__file__).parents[5] / "web" / "build")
LOCAL_HOSTS = ["127.0.0.1", "localhost"]


class Dashboard(StaticFiles):
    """The built dashboard. A page path with no file of its own (a story: /sessions/<id>/) gets the
    fallback page of the static build, which routes in the browser; /api paths keep their 404."""

    FALLBACK = "200.html"

    async def get_response(self, path: str, scope: Scope) -> Response:
        try:
            return await super().get_response(path, scope)
        except StarletteHTTPException as e:
            if e.status_code != 404 or path.split("/", 1)[0] in ("api", "ingest") or "." in path.rsplit("/", 1)[-1]:
                raise
            return await super().get_response(self.FALLBACK, scope)


def _version() -> str:
    try:
        return package_version("gitprompt")
    except PackageNotFoundError:
        return "0.0.0"


def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or Settings()
    settings.token()  # create the hook secret up front
    store = Store(settings.db_path)

    @asynccontextmanager
    async def lifespan(_: FastAPI) -> AsyncGenerator[None]:
        # A rules change rebuilds the index before the first page. Broken spec files are skipped
        # (and reported on the Connectors page), so start-up never fails over one.
        Indexer(store, load_specs(settings)).ensure()
        retention_pass = asyncio.create_task(retention.keep(store, PreferenceFile(settings.home)))
        yield
        retention_pass.cancel()
        store.close()

    app = FastAPI(title="GitPrompt", version=_version(), lifespan=lifespan)
    app.state.settings, app.state.store, app.state.bus = settings, store, Bus()

    # Only answer to local host names: blocks DNS-rebinding attacks from web pages.
    app.add_middleware(TrustedHostMiddleware, allowed_hosts=LOCAL_HOSTS)

    @app.exception_handler(GitPromptError)
    async def domain_error(_: Request, error: GitPromptError) -> JSONResponse:
        return JSONResponse({"detail": str(error)}, status_code=error.status)

    for router in ROUTERS:
        app.include_router(router)

    @app.get("/healthz", include_in_schema=False)
    async def healthz() -> dict[str, bool]:
        return {"ok": True}

    if DASHBOARD.is_dir():  # last: it catches every other path
        app.mount("/", Dashboard(directory=DASHBOARD, html=True), name="dashboard")
    return app
