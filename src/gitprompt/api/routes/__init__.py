"""HTTP layer: thin routers that validate input, call the core modules and shape responses.

Read-only routers are open to the local dashboard; each `writes` router requires the X-GitPrompt
header (deps.require_dashboard), and the ingest router requires the hook token.
"""

from fastapi import APIRouter

from gitprompt.api.routes import data, fields, ingest, prompts, security, settings, sources, stats, stories

ROUTERS: list[APIRouter] = [ingest.router, prompts.router, prompts.writes, sources.router, sources.writes, stats.router, fields.router]
ROUTERS += [settings.router, settings.writes, data.router, data.writes, security.router, security.writes]
ROUTERS += [stories.router]
