# GitPrompt

Local-first capture of AI coding prompts. Rust daemon in `backend/` (Cargo workspace), SvelteKit
dashboard in `web/` (pnpm). The Python daemon in `legacy/python/` is frozen: never edit it.
Where each kind of change goes: `docs/extending.md`.

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
