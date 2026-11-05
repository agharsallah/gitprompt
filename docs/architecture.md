# GitPrompt — Architecture (M1: local capture)

> **v1 design.** [PLAN.md](../PLAN.md) v2 and [ADR-0008](adr/0008-universal-capture-declarative-sources.md) replace the hook dialects (§6.2), connectors and readers (§7.1), the separate raw-event inbox and the browser site adapters (§10). The rest (storage backends §4, merge keys §7.2, enrichment §8, sharing groundwork §12) stays the reference for later iterations.

Rationale for decisions lives in [adr/](adr/).

---

## 1. Style: hexagonal, two-stage pipeline

- **Core (domain + pipeline)** is pure Python with no IO. Inputs are `RawEvent`s, outputs are `PromptRecord`s. Every step is a pure function you can unit test with fixtures.
- **Adapters** at the edges: capture channels (hooks, file readers, browser), storage (SQLite by default, Postgres optional), delivery (HTTP API, dashboard).
- **Two stages**: `RawEvent` (bronze, append-only, exactly what we received) → `PromptRecord` (silver, normalized, merged, redacted). Replaying stage 2 over stage 1 is always possible.

```
capture adapter ──► RawEvent ──► [inbox] ──► parse ──► enrich ──► classify ──► redact ──► merge ──► [prompts]
                     (bronze)                 connector   git/project  origin      secrets    dedup     (silver)
```

---

## 2. Process model

A single long-running **daemon** (`gitprompt serve`) running on asyncio:

| Component | Runs as | Notes |
|---|---|---|
| HTTP server (FastAPI/uvicorn) | main loop | Ingestion, hook gateway, query API, static dashboard |
| Normalizer worker | asyncio task | Pulls unprocessed `raw_event` rows through `EventStore.claim_batch()`: one worker on SQLite, `FOR UPDATE SKIP LOCKED` on Postgres so N workers (or hosts) can share the load (§4.3) |
| File watchers | asyncio task (`watchfiles`) | One watcher over all enabled channel globs. Debounced 250 ms, then the file is handed to its reader. |
| Periodic sweeper | asyncio task | Every 5 min, re-scans all enabled file channels (safety net for missed FS events, sleep/wake) |
| Readers | thread pool | SQLite copies and large JSON parses are blocking → `asyncio.to_thread` |
| Spool drainer | on startup + every 30 s | Ingests `~/.gitprompt/spool/*.jsonl` written by `gp-hook` while the daemon was down |

**Why one process:** local-first, one thing to install and supervise. The ports/adapters split keeps the option to split later (e.g. a separate collector per machine syncing to a shared server).

**Durability guarantee:** an event is acknowledged only after it is durable locally: its `raw_event` row is committed (SQLite / local Postgres), or it is appended to the local spool (remote Postgres, §4.4). Everything downstream is idempotent and re-runnable.

---

## 3. Domain model

```python
class Surface(StrEnum):   cli, ide, ide_extension, web, desktop_app
class Channel(StrEnum):   hook, transcript, browser, export, manual
class Origin(StrEnum):    human, slash_command, subagent, system, simulated, unknown

class RawEvent(BaseModel):            # bronze — what we received, untouched
    id: UUID7
    connector_id: str                 # 'claude_code', 'copilot', 'cursor', ...
    channel: Channel
    dialect: str | None               # hook dialect or reader kind
    native_key: str                   # stable id of the record in the source (see §7)
    payload: dict                     # verbatim JSON (or decoded row)
    source_ref: SourceRef             # file path + offset/rowid, or hook endpoint
    observed_at: datetime
    host_id: UUID                     # this machine (future sync)

class PromptRecord(BaseModel):        # silver — normalized
    id: UUID7
    merge_key: str                    # dedup identity (see §7)
    connector_id: str; tool: str; tool_version: str | None
    surface: Surface; origin: Origin
    session_ext_id: str | None; message_ext_id: str | None
    created_at: datetime              # submit time, as reported by the tool
    text: str                         # what the user typed
    expanded_text: str | None         # tool-expanded version (context injected), optional
    model: str | None; mode: str | None          # 'agent'|'ask'|'edit'|'plan'…
    attachments: list[Attachment]     # file paths, rules, images (hash + path only)
    workspace: Workspace              # cwd, workspace_roots, git_root, branch, head, remote
    channels_seen: set[Channel]       # e.g. {hook, transcript}
    raw_event_ids: list[UUID]         # provenance
    redactions: list[Redaction]       # type + span, never the secret
```

---

## 4. Storage (SQLite by default, PostgreSQL optional — ADR-0001)

### 4.1 Backends

Selected by one URL in `~/.gitprompt/config.toml`, checked by `gitprompt doctor`:

```toml
[storage]
url = "sqlite:///~/.gitprompt/gitprompt.db"          # default, created on first run
# url = "postgresql+psycopg://me@localhost/gitprompt"  # local Postgres / Docker
# url = "postgresql+psycopg://me@ep-xyz.eu-central-1.aws.neon.tech/gitprompt?sslmode=verify-full"
#        password is never stored here: it lives in the OS keychain (`gitprompt storage use <url>`)
```

| Backend | When | Notes |
|---|---|---|
| `sqlite` (default) | one machine, zero install | WAL mode, `busy_timeout=5000`, `synchronous=NORMAL`, `foreign_keys=ON`. File `0600`. The daemon is the only writer. |
| `postgres` local | you already run Postgres / Docker | Same as below, minus the remote safeguards (loopback/unix socket detected). |
| `postgres` remote (cloud) | multi-machine history, groundwork for sharing | Neon, Supabase, RDS, Cloud SQL… Any PG 16+ with `pg_trgm`. Remote safeguards in §4.4 apply automatically when the host isn't loopback. |

### 4.2 Logical schema

Written in Postgres syntax for readability; the SQLAlchemy Core models map each type per dialect (§4.3). One Alembic history serves both.

```sql
create table host (id uuid primary key, hostname text, os text, created_at timestamptz default now());

create table connector_state (                  -- per machine: hooks/paths are local to a host
  host_id uuid not null references host,
  connector_id text not null,
  enabled bool not null default false,
  channels jsonb not null default '{}',         -- {"hook": true, "transcript": true}
  consented_at timestamptz, consent_version int, -- consent text version shown to the user
  installed jsonb not null default '[]',        -- what we patched: file, entry id, backup path
  settings jsonb not null default '{}',         -- redaction opts, backfill range, ...
  primary key (host_id, connector_id)
);

create table checkpoint (                       -- reader progress, one row per source file/db per machine
  host_id uuid not null references host,        -- required once several machines share one Postgres
  connector_id text, source_key text,           -- e.g. absolute path
  state jsonb not null,                         -- {inode, offset, size, head_hash} | {max_rowid} | {mtime, sha256}
  updated_at timestamptz default now(),
  primary key (host_id, connector_id, source_key)
);

create table raw_event (                        -- bronze, append-only
  id uuid primary key,                          -- UUIDv7 (time-ordered)
  host_id uuid not null references host,
  connector_id text not null, channel text not null, dialect text,
  native_key text not null,
  payload jsonb not null,                       -- remote Postgres: {"enc": "<ciphertext>"} (§4.4)
  source_ref jsonb,
  observed_at timestamptz not null,
  processed_at timestamptz, parser_version text, parse_status text, -- ok|skipped|failed
  unique (connector_id, channel, native_key)    -- idempotent ingestion
);
create index on raw_event (processed_at) where processed_at is null;

create table project (                          -- a git repo (or bare folder) identity
  id uuid primary key,
  git_remote_normalized text,                   -- e.g. github.com/org/repo (null for non-git)
  root_path text, display_name text,
  unique (git_remote_normalized, root_path)
);

create table session (
  id uuid primary key,
  connector_id text not null, ext_id text not null,
  project_id uuid references project,
  title text, started_at timestamptz, last_activity_at timestamptz,
  meta jsonb not null default '{}',
  unique (connector_id, ext_id)
);

create table prompt (                           -- silver
  id uuid primary key,
  merge_key text not null unique,
  session_id uuid references session,
  project_id uuid references project,
  connector_id text not null, tool text not null, tool_version text,
  surface text not null, origin text not null,
  created_at timestamptz not null,
  text text not null, expanded_text text,
  model text, mode text,
  cwd text, git_branch text, git_head text,
  attachments jsonb not null default '[]',
  meta jsonb not null default '{}',             -- connector-specific extras
  channels_seen text[] not null,
  redactions jsonb not null default '[]',
  visibility text not null default 'private',   -- groundwork for sharing
  content_sha256 text not null                  -- of normalized text (sharing/dedup across hosts later)
);
-- full-text / substring search: dialect-specific, see §4.3
create index on prompt (created_at desc);
create index on prompt (connector_id, created_at desc);
create index on prompt (project_id, created_at desc);

create table prompt_event (prompt_id uuid references prompt, raw_event_id uuid references raw_event,
                           primary key (prompt_id, raw_event_id));   -- provenance
```

### 4.3 Dialect seam (the only backend-specific code)

Everything else is portable SQLAlchemy Core. These spots live in `storage/sqlite.py` and `storage/postgres.py`:

| Concern | SQLite | PostgreSQL |
|---|---|---|
| `uuid` | `TEXT` (canonical string, sorts by time for UUIDv7) | `uuid` |
| `timestamptz` | `TEXT` ISO-8601 UTC (`…Z`), converted in the type decorator | `timestamptz` |
| `jsonb` | `JSON` (TEXT + `json_extract`) | `jsonb` |
| `channels_seen text[]` | `JSON` array | `text[]` |
| Full-text search | FTS5 external-content table `prompt_fts(text)` with `unicode61`, synced by triggers | `tsvector` generated column (`'simple'`) + GIN |
| Substring / fuzzy | second FTS5 table with the `trigram` tokenizer | `pg_trgm` GIN index |
| Partial index on unprocessed events | supported (`WHERE processed_at IS NULL`) | same |
| Normalizer claim | single worker; `UPDATE … RETURNING` on a batch of `processed_at IS NULL` rows inside one write transaction | `SELECT … FOR UPDATE SKIP LOCKED` → N workers, possibly on several hosts |
| Upsert | `INSERT … ON CONFLICT DO UPDATE` (SQLite ≥ 3.35) | same |
| Migrations | Alembic `render_as_batch=True` (table rebuild for `ALTER`) | plain Alembic |
| Semantic search (later) | `sqlite-vec` (to evaluate) | `pgvector` |

The `SearchIndex` port takes a parsed query (`terms`, `phrase`, `prefix`) and each backend translates it, so the API's `q` parameter behaves the same; only ranking differs.

### 4.4 Remote Postgres safeguards

Applied automatically when the storage host isn't loopback/unix socket:

- **Explicit opt-in:** `gitprompt storage use <url>` (or the dashboard Settings page) shows what will leave the machine and requires confirmation. `doctor` and the dashboard header show "Storage: remote (<host>)".
- **Transport:** `sslmode=verify-full` is required; plain or `require` without verification is refused.
- **Credentials:** password/token in the OS keychain (`keyring`), never in `config.toml`.
- **Raw payloads encrypted client-side:** `raw_event.payload` holds unredacted input, so it is stored as AES-GCM ciphertext with a key kept in the local keychain (exportable for a second machine). Silver `prompt` rows are already redacted and stay queryable. Default bronze retention drops to 7 days.
- **Spool-first writes:** the hook gateway acks after appending to the local spool (`~/.gitprompt/spool/`), not after the network round-trip; the spool drainer forwards to Postgres in batches. A slow or offline database never delays a tool. `doctor` shows the backlog.
- **Multi-machine:** each daemon registers its `host` row; `connector_state` and `checkpoint` are keyed by `host_id`; dedup works across hosts via `merge_key`/`content_sha256`.

### 4.5 Switching backends

`gitprompt storage migrate --to <url>` stops ingestion, runs migrations on the target, copies all tables in dependency order (ids are UUIDv7, so nothing is re-keyed), verifies row counts + a hash per table, then rewrites `config.toml`. The source is left untouched as a backup. Works SQLite → Postgres and back.

---

## 5. Ingestion API (v1)

All endpoints live on `127.0.0.1:7878`, require `Authorization: Bearer <token>` (except `/healthz`), validate the `Host` header ∈ {`127.0.0.1:7878`, `localhost:7878`}, and accept CORS only from the paired extension origin.

| Method & path | Purpose | Response |
|---|---|---|
| `POST /v1/hooks/{connector}?dialect=…` | Hook gateway. Body = the tool's raw hook stdin JSON. | The dialect's **no-op response** (e.g. `{}`), always 2xx, within 50 ms |
| `POST /v1/events` | Generic batch ingestion (extension, spool drainer, importers in other processes) | `202 {accepted, duplicates}` |
| `POST /v1/imports` (multipart) | Upload an export file (Takeout, etc.) | `202 {job_id}` |
| `GET /v1/prompts` | Cursor pagination, filters: `connector, project, branch, model, origin, q, from, to` | `Page[PromptOut]` |
| `GET /v1/prompts/{id}` | Detail + provenance (raw events) | `PromptDetail` |
| `GET/POST /v1/connectors[/{id}/…]` | detect · consent · enable · disable · backfill · purge | — |
| `POST /v1/pair` | Extension pairing (one-time code shown in the dashboard → scoped token) | `{token}` |
| `GET /healthz`, `/metrics` | Ops | — |

The OpenAPI document generated by FastAPI is the contract. `web/packages/contracts` is generated from it (ADR-0006).

---

## 6. Hook gateway (ADR-0004)

### 6.1 Delivery modes
| Mode | Used by | How |
|---|---|---|
| **HTTP hook** (no process) | Claude Code (`type: "http"`); Copilot if its HTTP hook shape fits (Phase 0) | Tool POSTs stdin JSON directly to `/v1/hooks/{connector}` |
| **Command shim** `gp-hook <connector>` | Gemini CLI, Cursor, Antigravity, Copilot (fallback) | stdlib-only Python: read stdin → POST (300 ms timeout) → on failure append to spool → print no-op → **exit 0** |

### 6.2 Dialects are data
```python
DIALECTS = {
  "claude": Dialect(               # Claude Code, Copilot (PascalCase/snake_case), Gemini CLI, VS Code Copilot
    session="session_id", prompt="prompt", cwd="cwd", transcript="transcript_path",
    prompt_id="prompt_id", event="hook_event_name", ts="timestamp",
    prompt_events={"UserPromptSubmit", "BeforeAgent"}, noop={}),
  "copilot_camel": Dialect(        # Copilot userPromptSubmitted (camelCase)
    session="sessionId", prompt="prompt", cwd="cwd", ts="timestamp",
    prompt_events={"userPromptSubmitted"}, noop={}),
  "cursor": Dialect(
    session="conversation_id", message="generation_id", prompt="prompt", model="model",
    tool_version="cursor_version", workspace_roots="workspace_roots",
    transcript="transcript_path", attachments="attachments",
    prompt_events={"beforeSubmitPrompt"}, noop={"continue": True}),       # verify in Phase 0
  "nudge": Dialect(                # Antigravity: no prompt text → schedule a transcript read
    session="conversationId", transcript="transcriptPath", model="modelName",
    workspace_roots="workspacePaths", prompt=None, prompt_events={"PreInvocation"}, noop={}),
}
```
- The **mapper is generic**: `payload × Dialect → HookSignal` (session, prompt?, cwd, transcript_path, model, ids).
- A `HookSignal` with `prompt` becomes a `RawEvent(channel=hook)`. In every case, if `transcript_path` is present, the transcript reader for that file is **triggered immediately**. Hooks therefore also make the transcript channel real-time (ADR-0002).
- **Caller fingerprinting:** the gateway trusts the payload over the URL. `cursor_version` present → Cursor, `conversationId` + `artifactDirectoryPath` → Antigravity, and so on. This stops a Claude-format hook loaded by another tool (Copilot reads `.claude/settings*.json`, Cursor loads Claude Code hooks) from being misattributed. Repeats are absorbed by the merge step.
- Hooks are registered only on **prompt-submit events** (+ `SessionStart` where it carries metadata). We never register blocking tool hooks such as `PreToolUse`.

### 6.3 Installers (`JsonConfigPatcher`)
One class for every JSON config file. Per tool, only a *shape template* changes:

| Tool | File (user-level) | Entry |
|---|---|---|
| Claude Code | `~/.claude/settings.json` | `hooks.UserPromptSubmit[] += {hooks:[{type:"http", url, timeout:2, headers:{Authorization:"Bearer $GITPROMPT_TOKEN"}, allowedEnvVars:["GITPROMPT_TOKEN"]}]}` (or token inline, Phase 0 decision) |
| Copilot (CLI + VS Code) | `~/.copilot/hooks/gitprompt.json` (**own file**) | `{version:1, hooks:{userPromptSubmitted:[{type:"command", bash:"gp-hook copilot", timeoutSec:2}]}}` |
| Gemini CLI | `~/.gemini/settings.json` | `hooks.BeforeAgent[] += {hooks:[{type:"command", command:"gp-hook gemini_cli", name:"gitprompt", timeout:2000}]}` |
| Cursor | `~/.cursor/hooks.json` | `{version:1, hooks:{beforeSubmitPrompt:[{command:"gp-hook cursor", timeout:2}]}}` |
| Antigravity | `~/.gemini/config/hooks.json` | `PreInvocation` → `gp-hook antigravity` (shape verified in Phase 0) |

Patcher guarantees: parse-or-abort (never overwrite a file we can't parse), **timestamped backup**, entries **tagged** (`name: "gitprompt"` or a marker in the command), idempotent install, uninstall that removes *only* our entries, atomic write (temp + rename), file mode preserved. The diff is shown to the user in the consent dialog before writing.

---

## 7. Readers, checkpoints, merge & dedup

### 7.1 Generic readers (ADR-0005)
| Reader | Checkpoint | Edge cases handled |
|---|---|---|
| `jsonl_tail` | `{inode, offset, size, head_hash}` | partial last line (buffered until `\n`), truncation (size < offset → restart), rotation/replacement (inode or head hash changed → restart), invalid JSON line (counted, skipped) |
| `json_snapshot` | `{mtime_ns, size, sha256}` | file rewritten while reading (re-read if mtime changes), large files (streaming parser `ijson` if > N MB) |
| `sqlite_snapshot` | `{high_water: rowid/timestamp, db_sha}` | copy `db`, `-wal`, `-shm` to a temp dir, then `file:…?mode=ro&immutable=1`. Never touch the original. Locked/busy → retry with backoff. |
| `json_patch_log` | delegates to `jsonl_tail` + an in-memory replayed document per session | VS Code chat mutation log: `kind 0` = full snapshot, `kind 1` = set at path `k`, `kind 2` = array splice/push at `k`. Unknown kinds are counted. |

A connector's `parse(record, ctx) -> Iterable[RawEvent]` receives decoded records from a reader. Connectors **never open files themselves**, which keeps them pure and fixture-testable.

### 7.2 Merge keys (deterministic dedup)
Rank 1 is preferred. The same prompt seen via hook and transcript must produce the **same** `merge_key`.

| Connector | Rank 1: native shared id | Rank 2 fallback |
|---|---|---|
| Claude Code | `prompt_id` (hook) = `promptId` (transcript) | session + sha(text) + ts±5s |
| Copilot | transcript `user.message.data.messageId`. Hook has none → | session + sha(text) + ts±5s |
| Gemini CLI | `session_id` + `messageId` (logs.json) | session + sha(text) + ts±5s |
| Cursor | `generation_id` ↔ bubble id (to verify) | `conversation_id` + sha(text) + ts±5s |
| Antigravity | `conversationId` + `step_index` | — (single channel for text) |
| Web | site conversation id + client message id | tab + sha(text) + ts±5s |

The fallback is implemented as a *candidate lookup* (`session, sha`, within a time window) rather than baked into the key, so clock skew between channels doesn't create duplicates. When the transcript record arrives after the hook record, it **enriches** the existing prompt (model, message id, attachments) and adds `transcript` to `channels_seen`.

### 7.3 Origin classification (per-connector rules, shared enum)
- Claude Code: `isSidechain` → subagent. `origin.kind`/`promptSource` distinguish typed vs injected. Slash-command expansions → `slash_command`.
- Copilot: `parentAgentTaskId` on `user.message` → subagent. `content` = typed, `transformedContent` = expanded.
- Cursor: `isSimulatedMsg` → simulated. `composerHeaders.isSubagent` → subagent.
- Default `unknown` rather than guessing `human`.

---

## 8. Enrichment & redaction

- **Git/project enrichment** (shared service): given `cwd`/`workspace_roots`, resolve `git_root`, `branch`, `HEAD`, normalized remote (`git@github.com:org/repo.git` → `github.com/org/repo`). Results are cached per path and invalidated by a `.git/HEAD` mtime check. Prefer values the tool reports itself (Copilot `session.start.context.headCommit`, Claude `gitBranch`). For backfill, the value is "as recorded" or flagged `inferred`.
- **Workspace mapping (VS Code family):** `workspaceStorage/<hash>/workspace.json` → folder URI. A shared resolver is parametrized by app dir (`Code`, `Code - Insiders`, `Cursor`, `Antigravity IDE`, …).
- **Redaction** runs before the silver store: provider-key regexes (`sk-…`, `ghp_…`, `AKIA…`, `AIza…`, JWTs, PEM blocks, `KEY=value` in `.env` style) + an entropy check for long tokens. Replaced with `[REDACTED:type]` and recorded in `redactions`. Bronze retention is configurable (default 30 days after successful normalization), with client-side encryption of `raw_event.payload` (AES-GCM in the daemon, key in the OS keychain) — optional on local storage, **mandatory on a remote Postgres** (§4.4). Done in the app, not in the database, so it works the same on both backends.

---

## 9. Consent & lifecycle

```
undetected ─► detected ─► (user reads explanation + diff) ─► consented ─► enabled(channels) ─► disabled ─► purged
```
- **Detect:** manifest `detect` paths + version probe (`claude --version`, `product.json`, …). Read-only.
- **Explain:** generated from the manifest: files read, hooks installed (exact diff), metadata kept, `privacy_notes`. The consent text is versioned (`consent_version`), and a material change asks again.
- **Enable:** choose channels (live / backfill) and backfill range (all / N days / from now).
- **Disable:** uninstall hooks (patcher), stop watchers, keep data. **Purge:** delete the connector's prompts, sessions, raw events and checkpoints.
- Same flows via CLI: `gitprompt connectors list|enable|disable|purge`, `gitprompt doctor`.

---

## 10. Browser extension (ADR-0007)

```
page (MAIN world)                 content script (ISOLATED)        service worker                 daemon
fetch/XHR wrapper ──postMessage──► validate + adapter.extract ──► queue (IndexedDB) ──POST──► /v1/events
DOM fallback observer ───────────┘                                retry/backoff, pairing token
```
- **Engine (written once):** request interception, DOM fallback, message passing, queue, pairing, per-site enable state.
- **Site adapter (data + a small TS function):**
  ```ts
  export const geminiWeb: SiteAdapter = {
    id: "gemini_web", matches: ["https://gemini.google.com/*"],
    request: { urlPattern: /BardFrontendService\/StreamGenerate/, extract: parseGeminiBody }, // verify in Phase 4
    dom: { input: "rich-textarea", submit: "button[aria-label*=Send]" },                      // fallback only
    conversationId: (url) => url.pathname.split("/app/")[1] ?? null,
  };
  ```
- Permissions: `optional_host_permissions` per site, requested at enable time. The browser shows its own consent prompt, and nothing is injected on non-enabled sites.
- Daemon access: `host_permissions: ["http://127.0.0.1:7878/*"]` from the service worker only. A pairing code exchanges for a scoped token (`events:write`).
- Tests: recorded request fixtures per site + Playwright against a local mock page.

---

## 11. Extensibility

- **Connectors:** Python entry points `gitprompt.connectors` → `ConnectorManifest` + parse functions + fixtures. The registry validates manifests at startup.
- **Dialects:** add a row to `DIALECTS` + a contract test.
- **Site adapters:** add a file in `apps/extension/src/sites/` + fixtures.
- **Exporters (later):** JSONL / Markdown / "prompt pack".

---

## 12. Groundwork for sharing (not built in M1)

Decisions made now so sharing doesn't require re-modeling later:
- **UUIDv7** everywhere (globally unique, time-ordered, merge-friendly across machines).
- `host_id` on every raw event, so multiple machines can sync into one store.
- `content_sha256` on prompts (cross-host dedup, "same prompt used in 3 repos").
- `visibility` column (`private` default). Sharing becomes a state change plus a sync adapter, not a schema rewrite.
- Normalized git remote on `project`, the natural join key for "team prompts for this repo".
- Silver records are append-and-enrich (never edited by the user in M1). User annotations (tags, titles) will live in separate tables, so they sync independently.
