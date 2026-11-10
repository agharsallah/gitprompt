# GitPrompt tasks. Run `just` to list them.
#
# The backend is the Rust workspace in backend/ (docs/adr/0023-rust-backend.md); the dashboard is web/,
# built into web/build and embedded in the `gitprompt` binary. The Python daemon in legacy/python is
# deprecated and only kept for comparisons (`just compare`).

bin := "backend/target/release/gitprompt"

default:
    @just --list --unsorted

# --- everyday ---

# Cargo caches build-script output keyed by paths relative to the workspace, but Tauri's build
# scripts write absolute paths into it: after the checkout is moved or renamed (e.g. rust/ ->
# backend/) a build fails on files under the old path. Every Cargo recipe runs this first; it
# remembers where backend/target was built and starts clean when that changes.
[private]
target-guard:
    #!/usr/bin/env bash
    set -euo pipefail
    stamp=backend/target/.checkout-path
    here="$(pwd)/backend"
    if [ -d backend/target ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" != "$here" ]; then
        echo "backend/ moved from $(cat "$stamp"): cleaning backend/target (cached absolute paths)"
        (cd backend && cargo clean)
    fi
    mkdir -p backend/target && printf '%s' "$here" > "$stamp"

# Install the web dependencies and fetch the Rust crates
setup: target-guard
    pnpm --dir web install
    cd backend && cargo fetch

# Build the dashboard, then the release `gitprompt` binary that embeds it
build: target-guard
    pnpm --dir web build
    cd backend && cargo build --release -p gitprompt-cli

# Run the daemon (receiver + dashboard) on http://127.0.0.1:7878
serve: build
    {{ bin }} serve

# Develop: the Rust daemon (debug; restarts on change with cargo-watch if installed) + Vite with hot reload on http://127.0.0.1:5173
dev: target-guard
    #!/usr/bin/env bash
    set -euo pipefail
    cd backend
    if command -v cargo-watch >/dev/null; then
        cargo watch -q -w crates -x 'run -q -p gitprompt-cli -- serve' &
    else
        echo "tip: cargo install cargo-watch to restart the daemon on change"
        cargo run -q -p gitprompt-cli -- serve &
    fi
    daemon=$!
    trap 'kill "$daemon" 2>/dev/null || true' EXIT
    cd .. && pnpm --dir web dev

# Install `gitprompt` into ~/.cargo/bin (with the dashboard embedded)
install: target-guard
    pnpm --dir web build
    cargo install --locked --path backend/crates/gitprompt-cli

# --- checks ---

# Everything CI runs: format, lint, Rust tests (unit, integration, end-to-end), web checks, tests and build
verify: lint test
    pnpm --dir web build

# Rust tests (all crates, end-to-end included) and web tests
test: target-guard
    cd backend && cargo test --workspace
    pnpm --dir web check
    pnpm --dir web test

# Formatting and lints, warnings are errors
lint: target-guard
    cd backend && cargo fmt --all --check
    cd backend && cargo clippy --workspace --all-targets -- -D warnings
    pnpm --dir web lint

# Format Rust and web code in place
fmt: target-guard
    cd backend && cargo fmt --all
    pnpm --dir web format

# Rust line coverage report in backend/target/llvm-cov/html (needs: cargo install cargo-llvm-cov)
cov: target-guard
    cd backend && cargo llvm-cov --workspace --ignore-filename-regex '(tests/|examples/|main\.rs)' --html

# Load benchmark: ingest throughput and dashboard read latencies on synthetic data (pass e.g. --prompts 5000)
bench *args: target-guard
    cd backend && cargo run --release -q -p gitprompt-server --example load -- {{ args }}

# Compare the legacy Python daemon and the Rust daemon endpoint by endpoint on a copy of a database
compare db="": build
    tools/compare-backends.sh {{ db }}

# --- debugging (the installed data in ~/.gitprompt, or GITPROMPT_HOME) ---

# Everything needed to debug an install: paths, daemon, database, index, connectors, spool
doctor:
    -{{ bin }} doctor

# Per connector and app: the newest captured prompt and how it was attributed
check:
    {{ bin }} check

# Run any gitprompt command with the release binary, e.g. `just gp compact --apply`
gp *args:
    {{ bin }} {{ args }}

# --- desktop app (backend/apps/desktop, Tauri v2) ---

# Desktop app in debug, with the `gitprompt` hook forwarder built next to it
desktop-dev: target-guard
    pnpm --dir web build
    cd backend && cargo build -p gitprompt-cli && cargo run -p gitprompt-desktop

# Desktop installers in backend/target/release/bundle; tauri builds the dashboard first (needs: cargo install tauri-cli --version "^2")
desktop-build: target-guard
    cd backend && cargo build --release -p gitprompt-cli
    cd backend && mkdir -p apps/desktop/binaries && cp target/release/gitprompt{{ if os_family() == "windows" { ".exe" } else { "" } }} apps/desktop/binaries/gitprompt-$(rustc -vV | sed -n 's/^host: //p'){{ if os_family() == "windows" { ".exe" } else { "" } }}
    cd backend/apps/desktop && cargo tauri build --config tauri.bundle.conf.json

# --- legacy Python daemon (deprecated, legacy/python) ---

# The legacy Python test suite
legacy-test:
    uv run --project legacy/python pytest -q legacy/python/tests

# Run the legacy Python daemon (its `gitprompt` otherwise hands every command to the Rust binary)
legacy-serve:
    uv run --project legacy/python gitprompt --legacy serve
