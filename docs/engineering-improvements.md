# Engineering improvements

Five points each for developer experience, verifiability, and keeping the Tauri app bug-free and
fast, plus four on measuring code complexity and structure. They are based on the repo as of
2026-11-12.

The basics are already in place: a pinned toolchain, strict clippy (`unwrap` and `panic` denied,
`unsafe` forbidden), `cargo-deny` and `cargo-machete`, a lean release build, blocking work kept off
the async threads, and the Tauri plugin tested with `tauri::test::mock_builder`. So these points
cover what is missing.

## Developer experience

1. **Hot reload inside the desktop window.** `just desktop-dev` loads the already-built `web/build`,
   so every UI change needs a rebuild. In dev, point the window at Vite (`127.0.0.1:5173`) and
   forward `/api` to the daemon. You then work in the real desktop window with instant reload.
2. **Faster feedback while editing.** Add `bacon`, which re-runs clippy and tests on save, and
   `cargo-nextest`, which runs tests in parallel and gives clearer failure output. That makes the
   strict lint setup feel quick rather than slow.
3. **Catch problems before CI does.** Add a pre-commit hook (e.g. `lefthook`) that formats and lints
   only the changed files. Today the gate is `just verify` or CI, which means a late round trip.
4. **Proper linting for the web code.** It is only Prettier and `svelte-check` today. Add ESLint with
   the Svelte plugin to catch unused code, accessibility issues and Svelte mistakes. `knip` finds
   unused exports and files.
5. **Remove misleading context.** `web/src/lib/api/client.ts` and `web/src/lib/api/types.ts` still
   describe the FastAPI `.py` files, which now live in the frozen `legacy/python`. Point those
   comments at the Rust code and its specs.

## Verifiability

1. **Generate the TypeScript types from Rust.** `web/src/lib/api/types.ts` is about 700 hand-written
   lines. Generate it with `ts-rs` or `tauri-specta`, and have CI fail when the checked-in file
   differs from a fresh build. A renamed field then fails the build instead of the dashboard.
2. **Snapshot every API response.** Use `insta` to record each endpoint's JSON for the fixtures in
   `backend/testdata/fixtures/`. Any change to what the API returns then shows up as a reviewable
   diff. Once those snapshots exist, `just compare` (which checks the Rust daemon against the
   deprecated Python one) can eventually go.
3. **Property-based tests on the parsers.** `proptest` is already a dependency. Use it on the
   source-spec mapping, redaction, git-context parsing and the spool. These take outside input and
   are where unexpected input breaks things.
4. **Coverage in CI, with a minimum.** `just cov` exists but only runs locally. Run it in CI and fail
   the build if coverage drops for the `capture`, `store` and `server` crates.
5. **Treat each spec as the test.** Every rule in `backend/crates/gitprompt-core/specs/` should have a
   test that names it, so "is this verified?" is answered by a search rather than by memory.

## Keeping the Tauri app bug-free and fast

1. **Smaller security surface, fewer odd bugs.** `backend/apps/desktop/tauri.conf.json` sets
   `csp: null` and `withGlobalTauri: true`. The capability in
   `backend/apps/desktop/capabilities/default.json` grants the native commands to any local port.
   - Grant them only to the daemon's actual address, decided at startup.
   - Set a real CSP.
   - Import `@tauri-apps/api` instead of relying on the global.
   - Stop the window from navigating anywhere except the daemon's address, and open external links
     in the system browser.
2. **Free the webview when the window is hidden.** Closing the window only hides it
   (`backend/apps/desktop/src/window.rs`), so a web engine using roughly 100 MB or more stays loaded
   all day for a tray app. Destroy the window on close and rebuild it on `show()`. The code already
   creates it on demand, so this change is cheap.
3. **Limit how often the backend pushes events.**
   `backend/crates/gitprompt-tauri/src/lifecycle.rs` sends a `gitprompt://captured` event for every
   capture. A burst of hooks floods the UI with events and refreshes. Combine them into at most one
   event every 250 to 500 ms, carrying the counts.
4. **Test on Windows and run the real app in CI.** CI runs on Ubuntu and macOS only and never builds
   the desktop app. Windows uses a different web engine (WebView2) and is where Tauri-specific bugs
   tend to appear. Add:
   - a debug desktop build on all three systems
   - a `tauri-driver` smoke test: launch, the dashboard loads, one native call returns a result
5. **Performance limits and safe updates.** Make CI fail when these get worse:
   - `just bench` speed (p95 latency)
   - time until the window is ready
   - idle memory use
   - web bundle size

   There is no updater or code signing configured yet. Add the updater plugin with signed releases
   so fixes reach users without a manual reinstall.

## Code complexity and structure

1. **Complexity limits on the Rust code.** Clippy can already measure this; the limits are just not
   set. Add `cognitive-complexity-threshold`, `excessive-nesting-threshold` and
   `too-many-lines-threshold` to `backend/clippy.toml`, and enable `cognitive_complexity`,
   `excessive_nesting` and `too_many_lines` in `[workspace.lints.clippy]`. CI's `-D warnings` then
   enforces them. A trial run on 2026-11-12 with limits of 15 / 4 / 60 found:
   - 26 blocks nested too deeply, mostly in `gitprompt-insights/src/difflib.rs`,
     `gitprompt-server/src/service.rs`, `gitprompt-store/src/index/mod.rs` and
     `gitprompt-store/src/store/edits.rs`
   - 4 functions over the complexity limit: `gitprompt-core/src/spec.rs:722` (18),
     `gitprompt-capture/src/forwarder.rs:126` (17), `gitprompt-capture/src/spool.rs:286` (17) and
     `gitprompt-store/src/db/mod.rs:46` (16)
   - 1 function too long: `gitprompt-insights/src/indexer.rs:149` (70 lines)

   The server crate did not compile at the time, so it was only partly checked. Fix these first, or
   start the limits just above today's worst case and lower them over time. `cognitive_complexity`
   is an experimental lint with some false positives, so give it the loosest limit.
2. **Complexity limits on the web code.** Once ESLint is in (developer experience 4), turn on
   `complexity`, `max-depth`, `max-nested-callbacks`, `max-lines-per-function`, `max-params` and
   `sonarjs/cognitive-complexity` (from `eslint-plugin-sonarjs`). With the Svelte plugin these also
   cover the `<script>` blocks in `.svelte` files. The largest files today are
   `web/src/routes/design/+page.svelte` (721 lines) and `web/src/lib/prompts/PromptDetail.svelte`
   (596).
3. **Enforce the dependency direction.** Nothing stops a dependency cycle or a layer importing the
   wrong way.
   - Rust: `cargo modules dependencies --acyclic` fails on module cycles inside a crate, and
     `cargo depgraph --workspace-only` draws the crate graph so the core → store → server layering
     can be checked. `cargo tree -d` lists third-party crates pulled in at more than one version.
   - Web: `dependency-cruiser` understands `.svelte` files and the `#lib/*` alias. Give it rules such
     as "no cycles" and "`lib` never imports from `routes`", and run it in CI.
4. **A `just metrics` report.** Checks catch regressions; a report shows the trend. One recipe that
   runs `scc --by-file` (lines and rough complexity for both languages), `rust-code-analysis-cli`
   (cyclomatic and cognitive complexity, Halstead, maintainability index per function), the
   dependency graphs above, and `rollup-plugin-visualizer` for what makes up the web bundle (the
   size limit itself is in Tauri 5). Keep the output so the numbers can be compared between releases.

## Where to start

These three give the most for the effort:

- types generated from Rust (verifiability 1)
- the security settings (Tauri 1)
- hot reload in the desktop window (developer experience 1)
