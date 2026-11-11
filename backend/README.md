# GitPrompt — Rust backend

GitPrompt's backend: the daemon, the `gitprompt` command and the desktop app's core, all in one
workspace. It replaced the Python daemon (now `legacy/python`, deprecated: [ADR-0023](../docs/adr/0023-rust-backend.md))
and keeps its database, HTTP API, JSON shapes, hook URLs and connector specs, so the dashboard in
`web/` and the hooks already installed in your tools work unchanged. Where each kind of change goes:
[docs/extending.md](../docs/extending.md).

## Crates

```
gitprompt-core      settings, Env injection, preferences, errors, JSON payload helpers, git facts (+ cache),
                    secret detection, connector specs (bundled YAML compiled in, SpecCache)
gitprompt-store     SQLite (Python-compatible schema): prompts, events, annotations, prompt index + FTS5,
                    typed filters, statistics, timeline, compact/retention, backup
gitprompt-capture   JSON + OTLP decoding, capture pipeline, probing/policy, connect/disconnect, connector
                    states, token rotation, offline spool, hook forwarder
gitprompt-insights  origin, surface, turn derivations, facts, lineage, stories, snippets, field catalog,
                    field coverage, indexer (injectable time zone)
gitprompt-export    redaction pass, report builders, Markdown / HTML / CSV / JSON / SVG (/ PNG) renderers
gitprompt-server    axum router (receiver + dashboard API + SSE), Service facade (Tauri commands call it
                    directly, no HTTP), Daemon (single instance, background retention, spool drain),
                    logging and request tracing, GET /api/status
gitprompt-dashboard the built dashboard (web/build), embedded in release builds, read from disk in debug
gitprompt-cli       `gitprompt` binary: serve, compact, reindex, check, doctor, hook (forwarder), spool drain
gitprompt-tauri     Tauri v2 plugin: the daemon in-process, live-update events, native commands over Service
apps/desktop        `gitprompt-desktop`: the Tauri app (window, tray, autostart, single instance)
```

Dependency order: core → store → capture / insights → export → (dashboard) → server → cli / tauri → desktop.

## What changes compared to Python

- **Native for Tauri:** `gitprompt_server::Service` exposes every route as a typed async method; the HTTP
  routes are thin adapters over it. Bundled specs are compiled into the binary.
- **No lost prompts:** `gitprompt hook <source>` forwards a hook payload and spools it to
  `~/.gitprompt/spool/` when the daemon is down; the daemon drains the spool at start.
- **Single instance:** the daemon holds `~/.gitprompt/daemon.lock`; the desktop app and the CLI can't
  both write.
- **Performance:** a writer connection plus a reader pool in WAL mode (reads never wait for ingest), batched
  OTLP inserts, specs reloaded only when a user spec file changes, git context and tool versions cached.
- **Self-contained:** the dashboard is embedded in the binary (`GITPROMPT_DASHBOARD=<dir>` serves
  another build); a binary built without it serves a page saying how to build it, and the API still works.
- **Fast reads:** a trigger-maintained `event_stats` table answers the Data and Connectors pages without
  scanning every event (2–3 s → 1–120 ms on a 470 MB database; see ADR-0023 for the measurements).
- **Typed everywhere:** filters, group-bys, outcomes and connector states are enums, so an unknown filter is
  a compile error, not a runtime error.
- **Deterministic tests:** time, time zone, git, tool versions and environment are all injected; tests never
  touch the real home folder or the process environment.

## Desktop app

`apps/desktop` is a Tauri v2 shell around the `gitprompt-tauri` plugin:

- **Daemon in-process.** The plugin starts `gitprompt_server::Daemon` on Tauri's runtime, so AI tool hooks
  reach `http://127.0.0.1:<port>` exactly as with `gitprompt serve`, and stops it gracefully on exit. If a
  CLI daemon already holds `~/.gitprompt/daemon.lock`, the status is `external` and the window uses that
  daemon instead of failing.
- **Window + tray.** The main window loads the dashboard the daemon serves. Closing it hides to the tray;
  capture keeps running. `--hidden` (used by the login item) starts in the tray only. A second launch
  brings the running app forward.
- **Native commands** (`plugin:gitprompt|<name>`: `status`, `connect`, `disconnect`, `enable`, `disable`,
  `rotate_token`, `compact`, `export_to_file`, `open_data_folder`, `reveal_backup`) call `Service`
  directly; errors are `{status, detail}` like the HTTP API. Events: live-update notices and status
  changes.
- **Hook forwarder.** The `gitprompt` binary is bundled as a sidecar next to the app executable, so hooks
  run `gitprompt hook <source>` and spool while the app is closed.

```bash
just desktop-dev    # builds the dashboard and the forwarder, then cargo run -p gitprompt-desktop
just desktop-build  # installers in target/release/bundle (cargo install tauri-cli --version "^2")
```

The window shows the embedded dashboard (debug builds read `web/build` from disk, so a
`pnpm --dir web build` shows up on reload); `tauri.bundle.conf.json` bundles the sidecar from
`apps/desktop/binaries/gitprompt-<target-triple>`. The app logs to stderr and `~/.gitprompt/logs/desktop.log`.

## Testing, debugging, measuring

```bash
just verify        # fmt, clippy -D warnings, cargo test --workspace (incl. crates/gitprompt-cli/tests/e2e.rs,
                   # which drives the real binary), web checks and build: what CI runs
just cov           # line coverage report (cargo install cargo-llvm-cov)
just doctor        # an install's paths, daemon and lock, database and index, connectors, spool; `--json` too
just bench         # crates/gitprompt-server/examples/load.rs: ingest throughput, read latencies (p50/p95);
                   # `--concurrent 10` adds ingest and reads at once with a slow git
just perf          # the same with budgets (exit 1 when over): what CI's `perf` job runs
just compare       # legacy Python vs Rust, every endpoint, on a copy of ~/.gitprompt/gitprompt.db
```

### Catching performance problems early

A running daemon watches itself (`crates/gitprompt-server/src/health.rs`) and reports under
`health` in `GET /api/status`, which `gitprompt doctor` shows and turns into findings:

| Signal | Logged at `warn` when | Where it points |
|---|---|---|
| `gitprompt_store::writer` `waited for` / `held the database writer` | a write waited over 250 ms, or something kept the writer over 500 ms | `at=` file:line of the code that took it: slow work inside a transaction stalls every write and every read's index refresh |
| `gitprompt_server::http` `slow request` | a request took over 1 s | method and path; per-route p50/p95/max in the status |
| `gitprompt_server::health` | resident memory over 1 GiB or 3× (and 256 MiB) above its settled baseline; CPU over 80% of a core for 3 samples; new 5xx errors, slow requests or writer waits since the last sample | the message; each kind repeats at most every 15 min |

The daemon samples every 30 s and starts judging memory growth and CPU after 2 min, so start-up
work (index rebuild, spool drain) is not reported as a problem. `Limits` and the interval are
`DaemonOptions` fields.

Before a release, CI's `perf` job (`just perf` locally) fills a store, then ingests through two
hook streams with a 600 ms git while the dashboard reads, and fails when a read's p95 is over
budget, the writer was kept over 500 ms under load, or peak memory is over budget. It fails on
work moved inside a transaction (the bug that made `/api/prompts` take seconds on 2026-11-11), and
`crates/gitprompt-server/tests/contention.rs` pins the same rule deterministically.

| Variable | Effect |
|---|---|
| `GITPROMPT_HOME`, `GITPROMPT_PORT` | data directory (`~/.gitprompt`), port (`7878`) |
| `GITPROMPT_LOG_LEVEL` | a level (`debug`: every request with status and duration) or a filter (`gitprompt=debug,tower=warn`); default: info for `serve`, warnings for other commands |
| `GITPROMPT_LOG_FORMAT=json` | JSON log lines |
| `GITPROMPT_DASHBOARD` | serve the dashboard from this folder instead of the embedded one |

Connector fixtures shared by capture, insights and server tests are in `testdata/fixtures/<tool>/`.
Each crate's helpers are in `tests/common/mod.rs`.
