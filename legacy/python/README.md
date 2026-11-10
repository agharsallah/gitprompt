# Legacy Python daemon (deprecated)

This is GitPrompt's original FastAPI daemon, frozen. The backend is now the Rust workspace in
[`rust/`](../../rust/README.md) ([ADR-0023](../../docs/adr/0023-rust-backend.md)).

- `uv run --project legacy/python gitprompt <command>` hands the command to the Rust `gitprompt`
  binary (after one notice on stderr). It looks for `$GITPROMPT_BIN`, then `rust/target/release`,
  `rust/target/debug`, `~/.cargo/bin`, then `$PATH`.
- `--legacy` (or `GITPROMPT_LEGACY=1`) runs this Python daemon instead. It is kept only to compare
  both on the same data: `just compare` (see `tools/compare-backends.sh`).
- `just legacy-test` runs its test suite.

Don't change this code: connector specs, fixtures and every fix go to `rust/`
(`rust/crates/gitprompt-core/specs/`, `rust/testdata/fixtures/`). The specs and fixtures here are a
frozen copy.
