# 0023 — The backend is Rust; the Python daemon is deprecated

**Status:** accepted (implemented)
**Builds on:** [0001](0001-local-first-daemon-pluggable-storage.md) (local daemon), [0008](0008-universal-capture-declarative-sources.md) (declarative sources), [0012](0012-prompt-index-and-stats.md) (prompt index), [0018](0018-export.md) (export), [0022](0022-surfaces-and-desktop-apps.md) (surfaces, desktop apps)

## Context
GitPrompt is desktop first: it should run as a native app (Tauri) that starts with the machine and keeps capturing with its window closed, as well as a `gitprompt` command. The Python daemon (FastAPI, ~8k lines) needed a Python runtime next to the app, was slow to start (7 s on a 470 MB database), and scanned every stored event on pages shown on every visit. The dashboard, the connector specs, the database and the hooks already installed in users' tools had to keep working unchanged.

## Decision
1. **The backend is the Rust workspace in `backend/`**, one crate per former Python domain, behind one application layer (`gitprompt_server::Service`) that both the HTTP routes and the Tauri commands call:

   | Python (`legacy/python/src/gitprompt/`) | Rust (`backend/crates/`) |
   |---|---|
   | `core/` (config, preferences, errors, files, payload, git), `sources/spec.py`, `insights/redaction.py` | `gitprompt-core` (`settings`, `preferences`, `error`, `files`, `json`, `git`, `spec`, `secrets`, `numbers`) |
   | `storage/` (store, index, stats, timeline), `capture/compact.py`, `retention.py` | `gitprompt-store` (`store/`, `index/`, `stats/`, `timeline`, `housekeeping`, `sql/`) |
   | `capture/pipeline.py`, `otlp.py`, `sources/probe.py`, `installer.py`, `connection.py`, `rotation.py` | `gitprompt-capture` (+ `spool`, `forwarder`: new) |
   | `insights/` (origin, surface, derivations, facts, lineage, stories, snippets, catalog, captures, indexer) | `gitprompt-insights` (+ `coverage`: the field-coverage check as code) |
   | `export/` | `gitprompt-export` (PNG rendered natively) |
   | `api/` | `gitprompt-server` (axum routes, `Service`, `Daemon`) |
   | `cli.py` | `gitprompt-cli` (the `gitprompt` binary; + `hook`, `spool drain`, `doctor`) |
   | — | `gitprompt-dashboard` (the built dashboard, embedded), `gitprompt-tauri` (plugin), `apps/desktop` |

2. **Wire compatible.** Same database file and schema (the Rust daemon opens a Python-written `~/.gitprompt/gitprompt.db` as is), same port, paths, query parameters, JSON shapes and status codes, same hook URLs and install records, same `~/.gitprompt/sources/` overrides. `tools/compare-backends.sh` (`just compare`) runs both daemons on copies of one database and compares every endpoint the dashboard calls; on the author's database (661 prompts, 204k events) every endpoint matches.
3. **Deliberate differences**, each pinned by a test (the comparison normalizes them):
   - a connector's `last_at` is its latest prompt *or event* ("Last event" on its card), not its latest prompt;
   - `/api/sources` lists connectors by id (the dashboard groups layers itself);
   - a usage group with no name is keyed `"null"`, not the Python artifact `"None"`;
   - commits after a prompt carry `{hash, subject, author, time}` only (no zero counts), as Python sent them;
   - response-time quantiles round the exact value like Python's `round` (an early port rounded after multiplying and was off by 0.1);
   - a prompt's model (`facts.model`, the index `model` column) is the main thread's model, not the model of most calls of any kind (Python showed Haiku when its web search or title calls outnumbered Opus's), and a turn with only background or subagent calls shows none (`gitprompt-insights/tests/main_model.rs`). Not normalized by `just compare`: it changes every count by model, so on such turns the comparison reports the model (16 of 708 prompts on the author's database);
   - Claude Code messages from another session (`<cross-session-message …>`) are `agent_message` and its `[Cross-session idle notice]` is `task_notification`; Python's frozen specs call them `typed`. Task notifications and agent messages must be whole blocks (`text_block`), so a lookalike tag such as `<agent-messages>` stays typed (`gitprompt-insights/tests/origin.rs`, `machine_prompts.rs`). Not normalized by `just compare`: origin counts differ by those prompts (5 of 721 on the author's database);
   - CSV cells that would start a formula are neutralized; a default export names the tool (Python's redaction dropped its label); PNG export is rendered by the daemon.
4. **One addition to the schema**: `event_stats`, rows, bytes and newest time per (source, event), kept exact by triggers and backfilled once on first open. The Data page and the Connectors page (shown on nearly every visit) read it instead of scanning every event. It is additive: the Python schema is untouched, and the triggers also count rows the legacy daemon writes.
5. **The Python daemon moves to `legacy/python/`**, frozen. Its `gitprompt` entry point hands every command to the Rust binary (`exec`, same arguments) after a one-line notice; `--legacy` (or `GITPROMPT_LEGACY=1`) runs the old daemon, for `just compare`. Its bundled specs and fixtures are frozen copies; the sources of truth are `backend/crates/gitprompt-core/specs/` and `backend/testdata/fixtures/`.
6. **The dashboard builds into `web/build`** and is embedded in the binary (`gitprompt-dashboard`, rust-embed: read from disk in debug builds, embedded in release), so `gitprompt` and the desktop app are single files; `GITPROMPT_DASHBOARD=<dir>` serves another build.
7. **Offline capture**: hooks can call the bundled `gitprompt hook <source>` instead of curl; it spools a payload when the daemon is down, and the daemon ingests the spool at start with each payload's original time. The desktop app installs hooks in this form; `gitprompt serve` keeps the curl form.

## Measurements (release build, same 470 MB database, Apple Silicon)
| | Python | Rust |
|---|---|---|
| Start until `/healthz` (index rebuilt) | 7.3 s | 0.8 s (3.9 s once, the first time `event_stats` is backfilled) |
| `/api/sources` (most pages) | 1.6–2.1 s | 0.12 s |
| `/api/data` | 2.7–3.0 s | 0.001 s |
| `/api/prompts?limit=50` (warm) | 0.15 s | 0.08 s |
| Every dashboard endpoint, once (`just compare`) | 15.6–21.9 s | 8.1–10.0 s |

`just bench` measures ingest throughput and read latencies on synthetic data, to catch regressions.

## Consequences
- The Python code is no longer changed. It goes away once the desktop app has shipped and `just compare` has had no use for one release; until then `just legacy-test` keeps it runnable.
- New work lands in Rust only; the extension points are listed in [docs/extending.md](../extending.md).
- The dashboard keeps one API for the browser and the desktop app; native-only features are Tauri plugin commands.
