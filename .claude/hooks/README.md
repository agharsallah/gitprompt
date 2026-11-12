# Claude Code guardrails

The hooks that hold agent changes to the quality CI enforces: wired in `.claude/settings.json`.

| When | Hook | What it does |
|---|---|---|
| Before an edit (Edit, Write, MultiEdit, NotebookEdit) | `guard.sh` | Refuses frozen or generated files; asks the user before a guardrail changes (`protected.sh`); asks before lint levels in a Cargo manifest change. |
| Before a shell command | `guard-bash.sh` | The same protection for files written by a command (`sed -i`, redirections, `tee`, `mv`, `cp`, `rm`, scripts); asks before `--no-verify`. |
| After an edit | `web-lint.sh`, `rust-format.sh` | Formats the file (Prettier, rustfmt); ESLint findings go straight back to Claude. |
| Before Claude or a subagent finishes | `quality-gate.sh` | Runs the checks of every part the working tree changed (however it changed): blocks with the output until they pass. |

## What is protected (`protected.sh`)

Refused: `legacy/python/` (frozen) and `web/src/lib/api/generated.ts` (`just types` writes it).

Asks first: the architecture map (`rules.rs`), the dependency policy (`deny.toml`), lint, format and
type configs (`clippy.toml`, `rustfmt.toml`, `eslint.config.js`, `prettier.config.js`,
`.prettierignore`, `tsconfig.json`), `vite.config.ts` (coverage thresholds), API snapshots, CI
workflows, the `justfile`, and these hooks and their settings. A change there is a decision the user
approves, never a way to make a check pass.

## The quality gate (`quality-gate.sh`, `checks/`)

| Check | Runs when these change | What |
|---|---|---|
| `10-rust` | `backend/**/*.rs`, manifests, `Cargo.lock`, `clippy.toml`, `rustfmt.toml` | `cargo fmt --check`, the architecture rules, clippy `-D warnings` and the tests of each changed crate (the workspace when shared config changed), `generated.ts` in sync when a wire type changed |
| `20-deps` | manifests, `Cargo.lock`, `deny.toml` | `just deps`: cargo-deny (a new duplicate or a stale skip fails) and cargo-machete |
| `30-web` | `web/**` | Prettier and ESLint on the changed files, svelte-check, `pnpm test:coverage` (thresholds included) |
| `40-tooling` | `tools/**`, `.claude/hooks/**`, settings | the scripts parse, shellcheck, `test.sh` (the guards' own tests) |

A check passed for exactly the current changes is not run again (`target/quality/.gate/`), so
finishing twice costs nothing. When a check still fails after Claude's retry, the gate stops
blocking and warns the user instead; CI stays the final gate. The changed files are the whole
working tree, so your own uncommitted work is checked too.

## Changing them

- A protected file: one line in the `case` of `protected.sh`, and a case in `test.sh`.
- A check: a file `checks/NN-name.sh` defining `check_paths` (git pathspecs; `*` crosses folders)
  and `check_run` (receives the changed files, one per line; non-zero and output on failure).
- Run `bash .claude/hooks/test.sh` and `shellcheck -x .claude/hooks/*.sh .claude/hooks/checks/*.sh`;
  the gate's `40-tooling` check runs both when the hooks change.
