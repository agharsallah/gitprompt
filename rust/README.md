# GitPrompt — Rust backend

The Rust port of the Python daemon in `src/gitprompt`, built to run natively inside a Tauri desktop
app (and still as a standalone daemon + CLI). Same on-disk database, same HTTP API and JSON shapes as
the dashboard in `web/` expects, same declarative connector specs.

**Status: red phase.** Every public API is declared with `todo!()` bodies and covered by tests that
compile and fail (≈1,600 tests). Implementation turns them green crate by crate.

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
                    directly, no HTTP), Daemon (single instance, background retention, spool drain)
gitprompt-cli       `gitprompt` binary: serve, compact, reindex, check, hook (forwarder), spool drain
```

Dependency order: core → store → capture / insights → export → server → cli.

## What changes compared to Python

- **Native for Tauri:** `gitprompt_server::Service` exposes every route as a typed async method; the HTTP
  routes are thin adapters over it. Bundled specs are compiled into the binary.
- **No lost prompts:** `gitprompt hook <source>` forwards a hook payload and spools it to
  `~/.gitprompt/spool/` when the daemon is down; the daemon drains the spool at start.
- **Single instance:** the daemon holds `~/.gitprompt/daemon.lock`; the desktop app and the CLI can't
  both write.
- **Performance:** a writer connection plus a reader pool in WAL mode (reads never wait for ingest), batched
  OTLP inserts, specs reloaded only when a user spec file changes, git context and tool versions cached.
- **Typed everywhere:** filters, group-bys, outcomes and connector states are enums, so an unknown filter is
  a compile error, not a runtime error.
- **Deterministic tests:** time, time zone, git, tool versions and environment are all injected; tests never
  touch the real home folder or the process environment.

## Testing

```bash
just rust-test     # cargo test --workspace
just rust-lint     # fmt + clippy -D warnings
just rust-cov      # line coverage, fails under 100% (cargo install cargo-llvm-cov)
```

Connector fixtures shared by capture, insights and server tests are in `testdata/fixtures/<tool>/`.
Each crate's helpers are in `tests/common/mod.rs`.
