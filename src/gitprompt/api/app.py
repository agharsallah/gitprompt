from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as package_version
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.staticfiles import StaticFiles
from starlette.middleware.trustedhost import TrustedHostMiddleware

from gitprompt.api.bus import Bus
from gitprompt.api.routes import ROUTERS
from gitprompt.core.config import Settings
from gitprompt.core.errors import GitPromptError
from gitprompt.storage.store import Store

DASHBOARD = Path(__file__).parents[1] / "dashboard"  # built by `pnpm --dir web build`
LOCAL_HOSTS = ["127.0.0.1", "localhost"]


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
        yield
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
        app.mount("/", StaticFiles(directory=DASHBOARD, html=True), name="dashboard")
    return app
