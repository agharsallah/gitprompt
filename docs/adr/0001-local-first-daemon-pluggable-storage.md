# 0001 — Local-first single daemon, SQLite by default, Postgres (local or cloud) optional

**Status:** accepted for SQLite (iteration 1); Postgres deferred to iteration 5

## Context
Prompts are personal and may contain code and secrets, and the user wants everything to work locally with no install friction. Docker isn't guaranteed to be installed (it's missing on the author's machine), and requiring a Postgres server just to start collecting is too heavy for a personal tool.
At the same time, the user wants the option to host the store in a managed cloud Postgres (Neon, Supabase, RDS, Cloud SQL, …), e.g. to reach the same history from several machines — which is also the natural first step towards sharing (architecture §12).

## Decision
- One local daemon process (`gitprompt serve`, FastAPI on `127.0.0.1`) hosts ingestion, workers, watchers, the query API and the static dashboard. This doesn't change with the storage backend: **capture always runs locally**, only the database location changes.
- Storage is selected by one URL in `~/.gitprompt/config.toml`:
  - **`sqlite` (default):** a single file `~/.gitprompt/gitprompt.db`, created on first run. Zero install.
  - **`postgres` (opt-in):** any PostgreSQL 16+ reachable by URL — a local install, Docker, or a managed cloud database.
- Storage is accessed only through ports (`EventStore`, `PromptStore`, `CheckpointStore`, `SearchIndex`), implemented once with SQLAlchemy Core. Dialect-specific code is confined to a small, listed set of spots (full-text search, worker claiming, JSON/array columns — see architecture §4.3). The domain never knows which backend is in use.
- One Alembic migration history that must run on both dialects. CI runs the storage integration suite against both.
- Remote (non-loopback) Postgres is treated as **data leaving the machine**: explicit opt-in in the dashboard/CLI, TLS required, credentials in the OS keychain, raw-event payloads encrypted client-side, and hooks acknowledged from the local spool rather than waiting on the network (architecture §4.4).
- `gitprompt storage migrate --to <url>` copies everything between backends (UUIDv7 ids are portable), so the choice is reversible.

## Consequences
+ Zero-install default: `uv tool install gitprompt && gitprompt serve` just works.
+ The same daemon can point at a cloud Postgres for multi-machine history; `host_id` on every row already supports several machines writing to one store.
+ Postgres keeps its strengths when chosen: `tsvector` + `pg_trgm` search, JSONB, `SKIP LOCKED` workers, pgvector later.
− Two dialects to test and keep migrations compatible with → mitigated by the narrow dialect seam and a CI matrix.
− Search ranking differs slightly between SQLite FTS5 and Postgres `tsvector` → acceptable; the API contract is the same.
− SQLite is single-writer → fine for one daemon per machine (the daemon is the only writer; CLI commands go through the API).
− Semantic search later: pgvector on Postgres, `sqlite-vec` on SQLite (to evaluate then).

## Alternatives considered
- **Postgres only, three local runtime modes (native / Docker / bundled)** — the previous version of this ADR. Rejected: install friction for a personal tool, and the "bundled cluster" mode is fragile to supervise.
- **SQLite only:** simplest, but no path to multi-machine or team use without a separate sync service.
- **Cloud-only backend (hosted API):** rejected (local-first requirement). The cloud Postgres option is the user's own database, never a GitPrompt service.
- **Local SQLite + background sync to cloud Postgres (dual store):** more resilient offline, but a real sync engine (conflicts, cursors) is M2+ work. The spool-first write path in remote mode covers the offline case for M1.
