# 0006 — Pydantic as single source of truth for contracts

**Status:** deferred to iteration 5 (iteration 1 hand-writes a small `web/src/lib/api.ts`)

## Context
Three codebases consume the same shapes: the Python daemon, the Svelte dashboard and the browser extension. Hand-maintained duplicate types drift.

## Decision
- Domain and API models are defined once in Pydantic (`gitprompt-core`, API schemas in `gitprompt-server`).
- FastAPI emits OpenAPI. `openapi-typescript` generates `web/packages/contracts`. A typed fetch client (`openapi-fetch`) is used in dashboard and extension.
- CI regenerates and fails on diff. The hook dialect sample payloads also double as contract test fixtures.

## Consequences
+ One change propagates everywhere, and the type checker catches consumers.
− A generation step in the dev loop → `task gen` / pre-commit hook.
