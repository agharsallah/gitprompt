# GitPrompt

Local-first capture of AI coding prompts. Rust daemon in `backend/` (Cargo workspace), SvelteKit
dashboard in `web/` (pnpm). The Python daemon in `legacy/python/` is frozen: never edit it.
Where each kind of change goes: `docs/extending.md`.

## Architecture: enforced, changed only on purpose

- Crates are layers (core → store → capture / insights → export → server → cli / tauri → desktop),
  and the map is `backend/crates/gitprompt-core/tests/architecture/rules.rs`: which crate may use
  which, which crate owns each infrastructure crate (`rusqlite` → store, `axum`/`tokio` → server…),
  which crates do no I/O. Modules inside a crate never use each other in a loop. **`just arch`**
  checks it all in seconds; `cargo test` and a Stop hook run it too. `just quality` opens the
  code-quality dashboard (backend and web: tests, coverage, architecture score, lint, dependencies,
  performance, bundle, Lighthouse; `just quality architecture` refreshes one report). For a
  structural refactor, compare before and after. A new report is one file in
  `tools/quality/reports/<area>/` (`tools/quality/README.md`).
- Before writing code, put it in the lowest crate and module that can hold it (`docs/extending.md`).
- A finding means the code is in the wrong place. Move the shared piece down, or into a new small
  module both sides use, or invert the dependency with a trait. Never widen the map, merge modules
  or re-export across layers to make a check pass.
- A new crate, a new edge between crates or a new owner for an infrastructure crate is a design
  decision: ask the user first, then change `rules.rs` and `docs/extending.md` ("Architecture") in
  the same commit. Editing `rules.rs` always asks for approval.

## API types: generated, never hand-copied

- The dashboard's API types live in `web/src/lib/api/generated.ts`, **generated from the Rust wire
  types** (ts-rs). Never write or edit a wire type by hand in TypeScript, and never copy a Rust
  struct into `types.ts`, `settings.ts` or a component.
- To change what the API sends or accepts: change the Rust type (`backend/crates/gitprompt-server/src/model.rs`
  or the insights/store/core type it re-exports), then run **`just types`**, which regenerates
  `generated.ts` and type-checks the dashboard. Commit both.
- A new type the dashboard needs: derive `TS` on it (next to `Serialize`) and, if it is a new
  endpoint's answer or body, add it to `roots()` in `backend/crates/gitprompt-server/tests/ts_types.rs`.
  Give it the dashboard's name with `#[ts(rename = "...")]` when the Rust name differs.
- Optional on the wire means `#[serde(default, skip_serializing_if = ...)]` in Rust (`Blank::is_blank`
  for compacted summaries); an `Option` that is omitted, not `null`, also needs `#[ts(optional)]`.
- `web/src/lib/api/types.ts` re-exports the generated types and only adds what the client defines
  itself (filters, queries, narrower views of open-ended maps). `cargo test` fails while
  `generated.ts` is stale (`tests/ts_types.rs`).

## API values: snapshot-tested

- `backend/crates/gitprompt-server/tests/api_snapshots.rs` snapshots every read endpoint over the
  conformance fixtures (insta). After an intended API change run **`just snapshots`** and review the
  diff of `tests/snapshots/`; never accept snapshots you have not read.

## Checks

- `just verify` runs what CI runs. Use pnpm (never npm) for the web; `uv` for any Python.
- Keep code modular: small single-purpose files and functions, shared helpers over duplication.

## Lint findings: fix the cause, never silence it

- Never write `eslint-disable` (any form, in TS or in Svelte markup), `@ts-ignore`,
  `@ts-expect-error` or `@ts-nocheck`, and never type anything `any`. ESLint rejects all of them
  (`web/eslint.config.js`), and a hook lints every web file Claude edits.
- Fix the finding at its root: narrow `unknown` with type guards, give the value a real type
  (`unknown`, a generic, a type from `generated.ts`), handle the case the rule points at. A cast
  (`as`) only to quiet a rule is silencing too.
- If a rule is genuinely wrong for this codebase, turn it off once in `web/eslint.config.js` with a
  comment saying why, and say so to the user; never per line.
