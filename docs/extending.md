# Extending GitPrompt

Where each kind of change goes in the Rust backend (`backend/`), and the test that proves it. The
rules behind the design are in [PLAN.md](../PLAN.md) and the [ADRs](adr/README.md); this is the map.
Crates depend on each other in one direction: core → store → capture / insights → export → server →
cli / tauri. A change lives in the lowest crate that can hold it.

## Before and after a change

```bash
just verify     # fmt, clippy (warnings are errors), every Rust test (unit, integration, end-to-end), web checks and build
just bench      # ingest throughput and read latencies: run before and after anything on a hot path
just compare    # only while the legacy daemon exists: every endpoint against the Python daemon on a copy of your data
just doctor     # what an install looks like: paths, daemon, database, index, connectors, spool
```

Tests live next to what they test: `backend/crates/<crate>/tests/<module>.rs`, helpers in
`tests/common/mod.rs`. Tests never touch your home folder or the process environment: settings take
an `Env`, and time, time zone, git, tool versions and commands are injected (traits in
`gitprompt-capture::probe`, `gitprompt-server::options`).

## A connector (a new tool, or a fix to one)

A tool is data, not code.

1. A spec in `backend/crates/gitprompt-core/specs/<id>.yaml` (listed in `BUNDLED` in `gitprompt-core/src/spec.rs`). Shared defaults are in `_defaults.yaml`. Every key is documented on `Spec` in `spec.rs`: `detect`, `install` (`merge`, `reply`), `map`, `event_names`, `prompt_events`, `drop_events`, `origin`, `surface`, `tool_kinds`, `tool_files`, `usage`, `answer`, `capabilities`, `join`, `policy`, `discard`, `ignored`...
2. Conformance fixtures in `backend/testdata/fixtures/<id>/NN_<what>.json`: `{about, payload, expect: {stored_as, fields, origin, turn}}`, synthetic payloads in the tool's documented shapes. They are replayed by `gitprompt-capture/tests/conformance.rs` (stored as what, with which fields), `gitprompt-insights/tests/conformance.rs` (origin, typed text, turn) and `gitprompt-insights/tests/coverage.rs`, which fails on any payload key that is not mapped, derived, `shown` or `ignored` with a reason.
3. To patch a spec on one machine without a release: copy it to `~/.gitprompt/sources/`, edit it, use **Re-map stored events** on its card. Specs reload when a file there changes (`SpecCache`).

A new wire format is a variant of `Format` (`spec.rs`) and an arm in `gitprompt-capture::pipeline::decode`. A new join mode is a variant of `Join`, handled in `pipeline::join_session` and `Store::rejoin`.

## A derived value (something shown about a turn, a session or a prompt)

- About a turn: a small function in `gitprompt-insights/src/derivations/turn.rs` filling a field of `TurnSummary`, a line in its `DOCS` (the Fields page) and, if a filter or a chart needs it in SQL, a column of `IndexRow` (`gitprompt-store/src/index/`) filled in `insights::indexer::entry`. Payloads are read through `insights::raw` so the field-coverage test knows what is used.
- About a session: `derivations/session.rs`. About the typed text: `derivations/stats.rs`.
- Bump `CODE_VERSION` in `insights/src/indexer.rs` when what the index stores changes: every install rebuilds its index on next start.
- What every page shows about a turn is its facts (`insights/src/facts.rs`) and `web/src/lib/prompts/facts.ts`.

## A filter, a grouping, a statistic

- A prompt filter: a field on `PromptFilter` (`gitprompt-store/src/filter.rs`) and one function in `gitprompt-store/src/sql/filter.rs`; parse it from the query string in `gitprompt-server/src/query.rs`. It then works for prompts, stats, stories, search and export.
- A `has` condition: a variant of `Has` and its line in `sql/filter.rs`.
- A grouping: a variant of `GroupBy` and one line in `gitprompt-store/src/stats/groups.rs`, then `just types`.
- A measure: `stats/aggregate.rs` and a field on `Bucket`, then `just types`.

## An API route

A method on `Service` (`gitprompt-server/src/service.rs`, where the work happens, on a blocking thread), a handler in the matching `routes/<group>.rs` (parse, call, serialize; nothing else), its wire type in `model.rs`, and a test in `gitprompt-server/tests/<group>.rs` through the router. Read routes are open to the dashboard; a write route goes in that module's `writes()` so it needs the `X-GitPrompt` header. Errors are `gitprompt_core::Error`: their HTTP status is `Error::status`.

### Wire types and their TypeScript

The dashboard's API types are **generated** from the Rust wire types into `web/src/lib/api/generated.ts` (ts-rs); never write one by hand in TypeScript.

1. Derive `TS` next to `Serialize` on the wire type and on every type it contains. Name it as the dashboard does with `#[ts(rename = "...")]`. A type whose JSON differs from its fields (a custom `Serialize`) gets `#[ts(as = "TheWireStruct")]`, like the derivation summaries in `insights/src/derivations/serial.rs`.
2. A field left out of the JSON is `#[serde(default, skip_serializing_if = ...)]`, which makes it optional in TypeScript; an omitted `Option` also gets `#[ts(optional)]` so it reads `?: T`, not `?: T | null`.
3. A new endpoint's answer or body: add it to `roots()` in `gitprompt-server/tests/ts_types.rs`.
4. `just types` rewrites `generated.ts` and type-checks the dashboard. `cargo test` fails while the file is stale, so CI catches a forgotten regeneration.

`web/src/lib/api/types.ts` re-exports the generated types and adds only what the client builds itself (filters, queries, narrower views of open-ended maps).

### Snapshots of the JSON

`gitprompt-server/tests/api_snapshots.rs` ingests every conformance fixture and snapshots each read endpoint (insta, `tests/snapshots/`). Times, durations and temp paths are normalized. After an intended change, `just snapshots` rewrites them: review the diff before committing.

## A desktop (Tauri) command

A function in `gitprompt-tauri/src/commands.rs` calling `Service`, its name in `command_names.rs` (permissions are generated from that list) and a test in `gitprompt-tauri/tests/plugin.rs`. The dashboard calls it only when running inside the app; everything else goes through the HTTP API, which works in both.

## An export format or export kind

A format: a variant of `Format` (`gitprompt-export/src/formats.rs`, its metadata in `meta()`), a module that renders a `Report`, and one arm in `body()` (`gitprompt-export/src/lib.rs`). A kind: a builder in `gitprompt-export/src/build/` and its data in `collect.rs`. A key that holds typed text, identity or raw payloads goes in `redact.rs`, never in a format: redaction runs once, before any renderer.

## A preference, a secret kind, a CLI command

- A preference: a field on `Preferences` (`gitprompt-core/src/preferences.rs`) with its key in `KEYS` and a normaliser; its `GITPROMPT_<KEY>` variable comes for free. Then a row in a `web/src/lib/settings/*Section.svelte`.
- A secret kind: one `(name, pattern)` in `KINDS` (`gitprompt-core/src/secrets.rs`), with a test in `tests/secrets.rs`.
- A CLI command: a variant of `Command` in `gitprompt-cli/src/lib.rs` and a function in `commands.rs`, tested with `assert_cmd` in `gitprompt-cli/tests/`.

## Debugging

- `gitprompt doctor` (or `just doctor`): everything about an install, with a one-line fix for each problem; `--json` for scripts. `GET /api/status` is its machine-readable core.
- Logs: `GITPROMPT_LOG_LEVEL=debug gitprompt serve` logs every request with its status and duration (never bodies, query values or tokens); requests slower than a second are logged as warnings at the default level. `GITPROMPT_LOG_LEVEL` also takes a filter, e.g. `gitprompt=debug,tower=warn`; `GITPROMPT_LOG_FORMAT=json` prints JSON lines. The desktop app also writes `~/.gitprompt/logs/desktop.log`.
- A hook that seems lost: `gitprompt check` (newest prompt per tool and app), the connector's card (last event, events per hour), the Spool section of `gitprompt doctor` (what the forwarder queued while the daemon was down; `gitprompt spool drain` delivers it now).
- The prompt index is a cache: `gitprompt reindex` rebuilds it and the event summary from the raw rows.
