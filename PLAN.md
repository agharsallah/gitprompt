# GitPrompt — Plan v2

> **GitPrompt** keeps every prompt you write to an AI tool (CLI, IDE or web) with its metadata, **locally**, in one place.
> **v2 in one sentence:** capture only through interfaces the tools document for this purpose (hooks, the page you type in), describe each tool as a YAML file instead of code, and keep every raw payload so a tool update never loses data.

| Document | What it contains |
|---|---|
| **PLAN.md** (this file) | Scope, principles, terms-of-service guardrails, architecture, iteration 1, roadmap, risks |
| [docs/adr/0008](docs/adr/0008-universal-capture-declarative-sources.md) | The v2 decision: documented interfaces + declarative source specs (supersedes ADR-0004, 0005, 0007) |
| [docs/sources.md](docs/sources.md) | Per-tool research: hook payloads, config files, policy keys, evidence |
| [docs/architecture.md](docs/architecture.md) | v1 detailed design. Still the reference for later add-ons (transcript backfill, Postgres), superseded where it conflicts with this plan |

---

## 1. What changed from v1

| v1 | v2 | Why |
|---|---|---|
| Hooks **and** reading each tool's internal transcripts/SQLite stores | **Documented hooks only**; transcript backfill becomes an optional add-on later | Internal formats are undocumented and change with releases: they were most of the code and most of the maintenance |
| One Python connector package per tool (manifest + parse code) | **One YAML spec per tool** (detect, install, field mapping, policy), hot-reloaded, user-overridable | A tool update becomes a data edit, often no edit at all |
| Hook "dialects" as Python tables | Shared **fallback-path mapping** (`session_id` → `[session_id, sessionId, conversation_id…]`) | New tools that copy Claude Code's hook shape work out of the box |
| Per-site browser adapters (request interception) | One **generic composer-capture** extension (text box + Enter/Send), per-site override only if needed | No dependency on private web APIs |
| Raw inbox table + normalizer worker + Postgres from day one | One `prompt` table with the raw payload in the row; SQLite only | Same replay guarantee (`remap`), a fraction of the code |

## 2. Scope

**Iteration 1 (built, see §7):** Claude Code (terminal, IDE extensions, desktop app: they all read `~/.claude/settings.json`) → local daemon → live dashboard.

**Later iterations (§8):** Copilot CLI + VS Code and Codex (shipped), Gemini CLI, Cursor (each a YAML file); web LLMs through the generic extension; optional history backfill; optional Postgres; signed remote spec registry.

**Non-goals (permanent):** intercepting or proxying network traffic; reading credentials or tokens; changing, blocking or adding context to prompts; automating any tool or website; reverse-engineering encrypted or private formats; sending data anywhere off the machine (unless the user later picks their own remote database).

## 3. Principles

1. **Documented interfaces only.** A channel is allowed only if the tool documents it for user extensions: hooks in user settings, or the user's own browser page. These are public contracts the vendor maintains, so they change rarely and mostly by adding fields.
2. **A source is data.** `src/gitprompt/sources/bundled/<tool>.yaml` holds detection, the exact config merge to install, the field mapping and the policy checks. Specs are re-read on every use; a file in `~/.gitprompt/sources/` overrides the bundled one.
3. **Raw is always kept.** Every payload is stored verbatim next to the mapped fields. When a mapping is fixed, `remap` rebuilds the fields for all past events.
4. **One receiver.** Every channel posts to `POST /ingest/{source}`. Everything downstream (mapping, git enrichment, storage, live push) is shared.
5. **Consent is explicit and reversible.** Connect shows the exact diff of the tool's config file first, takes a backup, and disconnect removes only GitPrompt's entries.

## 4. Terms-of-service and policy guardrails

These are hard rules for every source, enforced in code and review. They are engineering guardrails, not legal advice: users remain bound by their own agreements and company policies (for example, rules on where prompts may be stored).

**Always**
- Use only extension points the vendor documents for end users (for Claude Code: [hooks](https://code.claude.com/docs/en/hooks) and the [OpenTelemetry export](https://code.claude.com/docs/en/monitoring-usage) in the user's own `settings.json`).
- Subscribe only to observational events. Never hook decision points (tool permission, model switch, MCP elicitation) or events where a hook changes behaviour (`WorktreeCreate`). The reason for each skipped event is in the spec (`not_captured`).
- Change a tool's config only after the user has seen the exact diff and confirmed. Keep a backup. Make it fully reversible.
- Respect organization policy. If managed settings turn user hooks off (`allowManagedHooksOnly`, `disableAllHooks`, `strictPluginOnlyCustomization`), GitPrompt shows that and **does not install**. If an HTTP-hook URL allowlist is set, GitPrompt warns and does not work around it. If telemetry already has an endpoint (organization or user), the telemetry source is blocked; it never redirects an existing pipeline.
- Keep data on the machine: bind `127.0.0.1`, no telemetry, no outbound calls.
- Answer hooks with an **empty 2xx**. For Claude Code this means "no output": the prompt is never blocked, rewritten or given extra context. Failures (daemon down, timeout) are non-blocking by the tool's own design.

**Never**
- Proxy, intercept or decrypt traffic between a tool and its AI provider; change base URLs; use or read OAuth tokens, API keys or credential files.
- Read or modify the tool's other private files, including its transcripts (GitPrompt only stores the `transcript_path` string it is given).
- Enable telemetry options that capture more than needed: `OTEL_LOG_USER_PROMPTS` (hooks already have the prompt) and `OTEL_LOG_RAW_API_BODIES` (whole API requests) stay off.
- Automate a tool or website, send requests on the user's behalf, or scrape AI responses.
- Install anything into project-level or managed settings. User-level only.
- Edit managed (organization) settings or try to bypass them.

**Web (iteration 3):** capture only the text the user types in their own browser, on sites they enable one by one. No automated requests, no response scraping, no API interception. Each site's terms are reviewed before it's listed as supported.

## 5. Architecture

```
 CLI / IDE tools                               Web LLMs (iteration 3)
 Claude Code ─hooks+OTLP─┐                    ┌── generic extension: Enter/Send in a text box
 Copilot, Gemini, Cursor ─┤ (iteration 2)      │   on a site you enabled → its text
                          ▼                    ▼
              ┌──────── gitprompt daemon · 127.0.0.1:7878 ─────────┐
              │ POST /ingest/{source}  (token, empty 2xx reply)    │
              │ POST /ingest/{source}/v1/{logs,metrics}  (OTLP)    │
              │   spec mapping → git context → SQLite → SSE push   │
              │ /api/sources  detect · policy · activity · connect │
              │ /api/prompts  list · filters · detail · notes      │
              │ /api/stats    counts · sums · quantiles, group_by  │
              │   split_by · order; /telemetry per session         │
              │ /api/stories  session chapters · project phases ·  │
              │   prompt versions; /api/search FTS + snippets      │
              │ /api/export   story · Overview · prompts → md,     │
              │   html, csv, json, svg (redacted, file only)       │
              │ /api/fields   seen vs documented fields            │
              │ /             dashboard (SvelteKit static build)   │
              └────────────────────────────────────────────────────┘
```

| Piece | File | Role |
|---|---|---|
| Settings | `src/gitprompt/core/config.py` | `~/.gitprompt/` (db, token, backups, user specs), host/port (env only) |
| Preferences | `src/gitprompt/core/preferences.py`, `api/routes/settings.py` | What the Settings page changes: `~/.gitprompt/preferences.json`, precedence env (`GITPROMPT_<KEY>`, locked) > file > defaults, each value reports its source; atomic writes; re-read on use so changes apply without a restart ([ADR-0013](docs/adr/0013-preferences.md)) |
| Source specs | `src/gitprompt/sources/spec.py`, `sources/bundled/*.yaml` | Schema, load + user override, `${VAR:-default}`/`~` paths. Each file loads on its own: a broken one is reported (`GET /api/sources/problems`) and skipped, and a broken override falls back to the bundled file. `capabilities` say what a connector yields, `join` how its events find their prompt ([ADR-0017](docs/adr/0017-connector-capabilities.md)). A spec can be a layer of another tool (`part_of`, `layer`, `summary`), so hooks and telemetry show as one tool ([ADR-0015](docs/adr/0015-connector-layers-and-states.md)) |
| Payload helpers | `src/gitprompt/core/payload.py` | Pure JSON helpers: fallback-path mapping (`pluck`, `extract`), `clip`, `flatten`, `map_strings` |
| Probe | `src/gitprompt/sources/probe.py` | Read-only checks on this machine: detection, tool version (cached), policy rules (`FILE_RULES`) |
| Connections | `src/gitprompt/sources/connection.py` → `installer.py` | Connect/disconnect workflow (policy gate, dry-run diff, record of added values) on top of the JSON mechanics: render, merge, strip (by receiver URL), parse-or-abort, diff, backup, atomic write keeping the file mode |
| Capture | `src/gitprompt/capture/pipeline.py`, `otlp.py` | One pipeline for every channel: decode by `spec.format` (`DECODERS`), map, skip events the spec doesn't keep (`accept_events`, `drop_events`), observe (git, version), store |
| Compact | `src/gitprompt/capture/compact.py`, `retention.py`, `cli.py`, `api/routes/data.py` | `gitprompt compact` and Settings › Data: deletes stored events the current specs would drop and, optionally, raw events older than N days or of one source/before a date (never prompts or notes), then `VACUUM`. Dry run unless applied ([ADR-0011](docs/adr/0011-drop-noise-events.md)). The `event_retention_days` preference runs `expired` at start-up and every 6 hours |
| Token rotation | `src/gitprompt/sources/rotation.py`, `api/routes/security.py` | New receiver token, then every connected source is reconnected; the dry run shows each diff (token as a placeholder); refused if a connected source is blocked |
| Store | `src/gitprompt/storage/store.py` | `prompt` (mapped fields, ingest context, raw), `event` (all other events, joined by prompt/session id), `annotation` (user notes); a view adds turn number and neighbours (numbered per session, only for the sessions shown); `remap` and `rewrite` run in a transaction. Filters (`FILTERS`, shared by the prompt list and stats) run on the prompt index |
| Prompt index | `src/gitprompt/storage/index.py`, `insights/indexer.py` | A rebuildable cache of `derivations.derive()` for SQL `WHERE`/`GROUP BY`: `prompt_index` (origin, local day/hour/weekday, usage, outcome, tool counts, repo, session), per-model usage and per-tool calls, FTS5 trigram over the typed text. Ingest marks turns that got events; the next read recomputes them. Rebuilt when the rules version (code hash + spec `origin`/`injected`/`tool_kinds`/`tool_files`/`usage` + timezone) changes, after remap and compact, and by `gitprompt reindex` ([ADR-0012](docs/adr/0012-prompt-index-and-stats.md)) |
| Stats | `src/gitprompt/storage/stats.py`, `api/routes/stats.py`, `api/filters.py` | `GET /api/stats`: the prompt filters plus `group_by` (`GROUPS`), `split_by` (each group's `parts`) and `order`; counts, sums, response-time p50/p90, a `detail` per group. `GET /api/stats/telemetry`: telemetry metrics and slash commands over the sessions the filters keep ([ADR-0014](docs/adr/0014-insights-stats.md)) |
| Git | `src/gitprompt/core/git.py` | State at prompt time (branch, HEAD, upstream ahead/behind, staged/modified/untracked, remote without credentials, last commit) and commits made after the prompt; `history` lists a story's commits with their diffstat in one call |
| Stories | `src/gitprompt/storage/timeline.py`, `insights/stories.py`, `insights/lineage.py`, `insights/snippets.py`, `api/routes/stories.py` | Ordered views of what you typed ([ADR-0016](docs/adr/0016-prompt-stories.md)): `timeline` reads index rows in time order with typed text and outcome; `stories` builds a session as chapters and system markers, a project as phases (`GAP_S` pause or a lasting branch change) with commits attributed to prompts, and a prompt's versions; `lineage` is text similarity (trigram + word Jaccard, `THRESHOLD`), word diff and outcome change; `snippets` highlights search matches. Pure functions, so an export can render the same data |
| Export | `src/gitprompt/export/`, `api/routes/export.py`, `web/src/lib/export/` | A story, the Overview or a prompt selection as a file you send yourself ([ADR-0018](docs/adr/0018-export.md)): one redaction pass (`export.redact`: secrets always hidden; prompt text off, relative paths, no account identity, no raw payloads by default), pure builders to a neutral `Report` and rows (`build.BUILDERS`), one renderer per format (`formats.FORMATS`: Markdown, self-contained HTML with inline SVG and its own CSP, CSV, JSON, one chart as SVG; PNG is drawn in the browser). The dashboard's preview shows the daemon's rendering and saves those bytes |
| Insights | `src/gitprompt/insights/derivations.py`, `origin.py`, `redaction.py`, `catalog.py` | Derived on read through three registries (`PROMPT_STATS`, `TURN`, `SESSION`): each value is one decorated function that also documents it for the Fields page; prompt origin and typed text from the spec's `origin`/`injected` rules ([ADR-0010](docs/adr/0010-prompt-origin.md)); secret detection/hiding; field catalog |
| API | `src/gitprompt/api/routes/*.py`, `deps.py`, `bus.py`, `app.py`; `core/errors.py` | Thin FastAPI routers, typed dependencies, domain errors mapped to HTTP in one handler, native SSE |
| Dashboard | `web/` (SvelteKit 3, Svelte 5 runes, adapter-static) | `lib/api/` (`types.ts` API types, `client.ts`, `live.ts` SSE), `lib/shared/` (`format.ts`, `task.svelte.ts` busy/error state, `MetaTable`), `lib/prompts/` and `lib/sources/` components; Prompts (live feed, filters, detail, notes, event timeline), Fields (catalog), Connectors (route `/sources`: one card per tool with its layers as steps; the state of each connector is derived from `/api/sources` facts in `lib/sources/state.ts`, one state -> icon, label, tone, action and sentence, shared by the Connectors page, Settings and first-run steps; consent review, heartbeat, what it collects, remap; `?connect=<id>` opens a review), Stories (`/sessions/[id]`, `/projects/[id]`, the prompt's Versions tab, `/search`; `lib/stories/`; dynamic routes are served by the static build's `200.html` fallback, which `api/app.py Dashboard` returns for page paths without a file) |

**Extending:** a new tool is a YAML spec plus a folder of conformance fixtures, `tests/sources/fixtures/<id>/` (synthetic payloads from its docs, each with the fields, origin and turn it must give; `tests/sources/test_conformance.py` replays them). It lists what it yields in `capabilities` (a new kind is a name in `sources.spec.Capability`, `Capability` in `web/src/lib/api/types.ts` and a row in `CAPABILITIES` of `web/src/lib/sources/capabilities.ts`); a panel that needs one calls `availability(sources, capability)` and shows `NotAvailable`/`CapabilityNote` from `lib/insights/panels/`. A tool that names files inside a text input (a patch) gets a `tool_files` pattern, which `insights.derivations.files_of` applies for turns, the index and stories; `assistant` names who answers in it and `after_connect` what the user still does in the tool once connected ([ADR-0019](docs/adr/0019-codex-connector.md)). Events that report one model call each are a `usage` block in its spec (event names and payload paths of model, tokens, cost, duration, error; default in `_defaults.yaml`), and a capability only newer versions have is a `capability_since` entry, which `/api/sources` turns into `needs_version` ([ADR-0020](docs/adr/0020-model-call-usage-and-version-capabilities.md)). A tool that sends no prompt id sets `join: session` (a new join mode is a name in `sources.spec.Join`, handled in `capture.pipeline` and `Store.rejoin`); an optional extra channel for the same tool (like telemetry) is another spec with `part_of` naming the main one, and shows as a layer of that tool. A new connector state is an id, a row in `STATES` and a fixture in `web/src/lib/sources/` (`state.ts`, `fixtures.ts`). A new wire format is a decoder in `capture.pipeline.DECODERS` plus its name in `sources.spec.Format`. A new derived value is one function registered on `insights.derivations.TURN` (or `PROMPT_STATS`/`SESSION`). What a tool does (read, write, shell, web, mcp, agent; else other) is data: a `tool_kinds` entry in its spec (`Bash: shell`, `mcp__*: mcp`), which sets the files, shell commands, git commits and URLs derived for a turn; a new kind is a name in `sources.spec.ToolKind` and in `TOOL_KINDS`/`KIND_INFO` of `web/src/lib/insights/tools.ts`. A new policy key is a field on `sources.spec.Policy` plus a rule in `sources.probe.FILE_RULES`; a settings file in another format is a parser in `sources.probe.PARSERS` (JSON by default, `.toml` read with `tomllib`). A new way to tell who produced a prompt is an `origin` rule in the tool's spec (shared rules in `_defaults.yaml`), and a new kind of tool-added context is a tag in its `injected` list; `insights.origin.classify` applies them on read ([ADR-0010](docs/adr/0010-prompt-origin.md)). A new secret kind is a pattern in `insights.redaction.SECRETS`. A new prompt filter is a line in `storage.store.FILTERS` (a SQL condition on the prompt index) plus a field on `api.filters.PromptFilters`; it then works for both `/api/prompts` and `/api/stats`. A value a filter or chart needs in SQL is a column in `storage.index` filled from `derivations.derive()` in `insights.indexer.entry` (the index rebuilds itself when that code changes). A new statistics grouping is a line in `storage.stats.GROUPS` plus its name in `GroupBy` (and in `GroupBy` of `web/src/lib/api/types.ts`); a value that depends on later prompts or the clock is SQL at query time (`store.FOLLOWED`, `store.OUTCOME`), not an index column. A new `has` condition is a line in `storage.store.HAS` plus its name in `api.filters.Has`. Telemetry without a prompt id is indexed per session (`session_metric`, `session_command`, read by event id in `Indexer._index_telemetry`). A new Overview panel is one component in `web/src/lib/insights/panels/` fed by one `/api/stats` call in `lib/insights/overview.ts`; its links go through `promptsHref` (`lib/insights/filters.ts`). A new user preference is a field on `core.preferences.Preferences` (its env var `GITPROMPT_<KEY>` comes for free; list it in `APPLIES_AT_START` if it needs a restart) plus a row in a `web/src/lib/settings/*Section.svelte`; a browser-only preference is an entry in `web/src/lib/settings/browser.ts`. A story is data from `insights.stories` rendered by components in `web/src/lib/stories/` (one per concern); a new phase rule is a change to `stories.phases`, a new similarity rule to `lineage.similarity` (tests in `tests/insights/`), and a new kind of system marker is a kind in `stories._turn_markers` plus a row in `MARKERS` (`web/src/lib/stories/story.ts`). A new export format is a module in `src/gitprompt/export/` with one `@formats.register(...)` renderer, imported in `export/__init__.py`; a new kind of export is a builder registered with `build.builder` plus its data collected in `api/routes/export.py`; a new key that holds typed text, identity or raw payloads goes in `export.redact` (`TEXT_KEYS`, `IDENTITY_KEYS`, `RAW_KEYS`), never in a format.

**Security:** `TrustedHostMiddleware` (127.0.0.1/localhost only, blocks DNS rebinding); `/ingest` needs a bearer token (`~/.gitprompt/token`, mode 0600); dashboard writes need an `X-GitPrompt` header, which browsers only send from the same origin because CORS is not enabled; data dir 0700, db 0600.

## 6. How tool updates are absorbed

| The tool… | What happens |
|---|---|
| adds metadata fields | Nothing to do: stored in raw, shown in the prompt detail automatically |
| renames a field to a common alias | Nothing to do: the shared fallback paths already cover it |
| renames a field to something new, or moves its config file | Edit one line of YAML (or drop an override in `~/.gitprompt/sources/`), then **Re-map stored events** |
| breaks silently | Drift signal: the Sources page counts events with no prompt and warns |
| drops its hooks | Optional add-ons (transcript backfill, OpenTelemetry) can take over for that tool only |
| documents new fields or events | Add them to `documented` in the spec; the Fields page shows which ones are actually seen |
| adds or renames a tool | Add it to `tool_kinds` in the spec (unmapped tools count as "other") |
| sends high-volume events nothing reads | Add them to `drop_events` with the reason; unknown events are still kept by default. `gitprompt compact` removes those already stored |

Spec format (the real Claude Code spec is in `src/gitprompt/sources/bundled/claude-code.yaml`):

```yaml
id: claude-code
name: Claude Code
detect: ["${CLAUDE_CONFIG_DIR:-~/.claude}"]
version_cmd: [claude, --version]
install:
  file: "${CLAUDE_CONFIG_DIR:-~/.claude}/settings.json"
  merge: { hooks: { UserPromptSubmit: [ { hooks: [ { type: http, url: "{{receiver}}/ingest/{{source}}", timeout: 2,
           headers: { Authorization: "Bearer {{token}}" } } ] } ] } }
prompt_events: [UserPromptSubmit]                # other events attach to the prompt
map: { permission_mode: [permission_mode] }      # on top of sources/bundled/_defaults.yaml
origin: [ { when: { text_prefix: "<task-notification>" }, origin: task_notification } ]  # before the shared rules
injected: [ide_selection, ide_opened_file]       # context blocks removed from the typed text
policy: { files: [...managed settings...], block_if: { disableAllHooks: [true], strictPluginOnlyCustomization: [true, hooks] },
          warn_if_set: [allowedHttpHookUrls] }
terms: "Uses Claude Code's documented hooks feature…"
documented: { Stop: [last_assistant_message, …] }  # shown on the Fields page
not_captured: { PreToolUse: "A decision point…" }
drop_events: { "hook_execution_*": "Not stored. …" }  # received but not stored (trailing * = prefix)
```

## 7. Iteration 1 — Claude Code end to end (done)

**Flow:** `gitprompt serve` → open `http://127.0.0.1:7878` → **Sources**: "Claude Code, detected v2.1.x" → **Connect** shows the diff for `~/.claude/settings.json` → **Confirm** (backup taken) → type a prompt in any Claude Code session (running sessions pick up the change: Claude Code reloads hooks from settings files) → it appears live in **Prompts** with session, prompt id, cwd, git branch/remote, permission mode, transcript path and the raw payload.

**Run it**
```bash
just setup      # uv sync + pnpm install
just build      # builds the dashboard into src/gitprompt/dashboard
just serve      # http://127.0.0.1:7878
just test       # pytest + ruff + svelte-check
just dev        # API with reload + Vite dev server (proxy to the API)
```

**Verified**
- 14 tests: mapping, path expansion, spec override, merge/strip round trip, invalid-JSON refusal, connect → ingest → disconnect restoring the exact original settings, token and host checks, dedup, event filtering, remap, policy block.
- Real run with Claude Code 2.1.316 (hook passed via `--settings`, so no file was touched): the prompt was captured with all fields and pushed over SSE. With a wrong token, Claude Code still answered normally (the failure was non-blocking, as documented).

**Exit criteria:** a prompt typed in Claude Code appears in the dashboard in under a second; connect/disconnect leave `settings.json` exactly as before; nothing is installed when policy disables hooks.

### 7b. Rich metadata (done, [ADR-0009](docs/adr/0009-metadata-events-telemetry-derived-on-read.md))

| Group | Fields | Source |
|---|---|---|
| Prompt text | chars, words, lines, approx tokens, slash command, `@` mentions, URLs, code blocks, questions, **possible secrets** (with "Hide it") | derived |
| Turn | turn number, time since previous prompt, response time, ended by Stop/StopFailure, error, final response, tool calls/failures/time, files written/read, Bash commands, web fetches/searches, MCP tools, subagents, tasks, notifications, permission denials, compactions, model switches | 20 hook events |
| Usage | API requests, models, input/output/cache tokens, cost, API time, API errors, rejected tool calls | telemetry (opt-in source) |
| Repository | root, worktree, branch/detached, HEAD, upstream ahead/behind, staged/modified/untracked/conflicts, remote (credentials stripped), `owner/repo`, host, last commit subject/time/author; **commits made after the prompt** with diffstat | `git` CLI |
| Session | events by type, first/last event, end reason, instructions loaded, config/cwd changes, metric totals (lines of code, commits, active time, tokens, cost), environment (app version, entrypoint, terminal, OS, account) | hooks + telemetry |
| Tool | version at prompt time | `claude --version` (cached) |
| Yours | star, 👍/👎, tags, note; filter by tag, star, session, repository | dashboard |

The **Fields** page lists every field path seen per source and event, with counts and the latest value, next to the fields each tool documents (also those not seen yet), the derived fields, and the events not captured with the reason.

**Verified:** 26 tests. A real Claude Code 2.1.317 run (hooks and telemetry passed via `--settings` in a scratch git repo, no settings file touched) produced the prompt, 5 hook event types and 17 telemetry event and metric types. All were joined on the prompt, including the commit Claude made with its diffstat, tokens and cost.

## 8. Roadmap

| Iteration | Content | New code |
|---|---|---|
| **2. Other hook-capable tools** | **Done:** Copilot CLI + VS Code (`~/.copilot/hooks/gitprompt.json`, [ADR-0017](docs/adr/0017-connector-capabilities.md)); Codex CLI (`$CODEX_HOME/hooks.json`, `prompt_id: [turn_id]`, hooks trusted by the user with `/hooks`; no telemetry layer, tokens from its `PostModelRequest` hook on 0.164+, [ADR-0019](docs/adr/0019-codex-connector.md), [ADR-0020](docs/adr/0020-model-call-usage-and-version-capabilities.md)). Next: Cursor (`beforeSubmitPrompt`); Gemini CLI is sunset for individuals. **No command shim:** the curl command forwarder already covers every researched tool (all run a shell command with the payload on stdin); a stdlib shim waits for Windows support or a tool that can only exec a binary. Payload fingerprinting, since some tools also load Claude Code's hook file. | Fingerprint field in specs |
| **3. Web** | MV3 extension: one content script on user-enabled sites; captures the focused text box on Enter/Send; posts to `/ingest/web` with a paired token. Optional per-site selector overrides. | Extension (~150 LOC) |
| **4. History (optional add-on)** | Transcript backfill per tool, off by default, isolated behind the same spec format (`backfill:` block). The only place that tracks private formats. | Readers from v1 architecture §7 |
| **5. Distribution & scale** | `uv tool install gitprompt`, auto-start (launchd/systemd), export, Postgres backend, signed remote spec registry, generated TS contracts (ADR-0006) | — |

## 9. Engineering standards

| Area | Standard |
|---|---|
| Python | 3.12+, uv, FastAPI (routers per domain, `Annotated` dependencies, Pydantic response models, native SSE), ruff, pytest. Stdlib `sqlite3` behind the `Store` class |
| Web | pnpm, SvelteKit 3 + Svelte 5 runes per [svelte.dev/llms.txt](https://svelte.dev/llms.txt): `$derived` over `$effect`, `load` functions for data, `$app/state`, keyed `#each`, `onclick` attributes, `refreshAll`, `#lib` imports |
| Tests | Synthetic payloads only (built from documented examples). Tests run in temp dirs and never touch real tool configs |
| Specs | Each spec links the vendor doc it relies on and carries a `terms` text shown before consent |
| Code size | Prefer data over code; a new source should need no Python |

## 10. Risks

| Risk | Mitigation |
|---|---|
| A tool changes its hook payload | Fallback paths, raw kept, drift warning, remap; spec override without a release |
| A tool's hook API is preview/experimental (VS Code Copilot, Gemini) | Marked in the spec; history add-on as fallback |
| Hook slows the tool | Empty 2xx in milliseconds, 2 s timeout, failures non-blocking |
| Token stored in the tool's user settings file | File is user-only (0600 kept); token only works against 127.0.0.1; rotate by deleting `~/.gitprompt/token` and reconnecting |
| Secrets inside prompts | Local only in iteration 1; redaction before any remote storage (iteration 5) |
| Organization forbids hooks | Detected from managed settings files and reported; server-managed or MDM policies can't always be read → the dashboard points to `/status` in Claude Code |

## 11. Open questions

1. Store the transcript path only (current), or offer the history add-on per tool in iteration 4?
2. Web sites for iteration 3: ChatGPT, Claude.ai, Gemini, Copilot — which first?
3. Should specs be fetchable from a signed remote registry (opt-in), or only shipped with releases plus local overrides?
