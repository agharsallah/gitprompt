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
