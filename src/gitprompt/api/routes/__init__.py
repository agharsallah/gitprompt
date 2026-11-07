"""HTTP layer: thin routers that validate input, call the core modules and shape responses.

Read-only routers are open to the local dashboard; each `writes` router requires the X-GitPrompt
header (deps.require_dashboard), and the ingest router requires the hook token.
"""

from fastapi import APIRouter

from gitprompt.api.routes import fields, ingest, prompts, sources

ROUTERS: list[APIRouter] = [ingest.router, prompts.router, prompts.writes, sources.router, sources.writes, fields.router]
