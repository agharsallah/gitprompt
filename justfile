# GitPrompt tasks. Run `just` to list them.

default:
    @just --list

# Install Python and web dependencies
setup:
    uv sync
    pnpm --dir web install

# Build the dashboard into the Python package (served by the daemon)
build:
    pnpm --dir web build

# Run the daemon on http://127.0.0.1:7878
serve: build
    uv run gitprompt serve

# API with auto-reload + Vite dev server proxying /api to it
dev:
    uv run uvicorn gitprompt.api.app:create_app --factory --host 127.0.0.1 --port ${GITPROMPT_PORT:-7878} --reload & pnpm --dir web dev

# Tests, lint and type checks
test:
    uv run pytest -q
    uv run ruff check src tests
    uv run ruff format --check src tests
    pnpm --dir web check
    pnpm --dir web test

# --- Rust backend (rust/) ---

# Rust tests (all crates)
rust-test:
    cd rust && cargo test --workspace

# Rust lint: formatting and clippy with warnings as errors
rust-lint:
    cd rust && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings

# Rust line coverage; fails under 100% (needs: cargo install cargo-llvm-cov)
rust-cov:
    cd rust && cargo llvm-cov --workspace --fail-under-lines 100 --ignore-filename-regex '(tests/|main\.rs)' --html
