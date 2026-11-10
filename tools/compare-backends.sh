#!/usr/bin/env bash
# Compare the legacy Python daemon and the Rust daemon on the same data, endpoint by endpoint.
#
#   tools/compare-backends.sh [DB]        # default DB: ~/.gitprompt/gitprompt.db (read-only)
#
# The database is copied twice (SQLite online backup, so a running daemon is fine): each daemon
# rebuilds its prompt index in its own copy, so neither ever writes to DB. Both run on free loopback
# ports with temporary data directories, and are stopped on exit. Exit status: that of
# tools/compare_backends.py (0 = every endpoint matches). Extra arguments after `--` go to it
# (e.g. `-- --only /api/stats --strict`).
#
# Needs: the Rust release binary (`just build`), uv, python3.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
db="${1:-$HOME/.gitprompt/gitprompt.db}"
[[ $# -gt 0 && "$1" != "--" ]] && shift
[[ "${1:-}" == "--" ]] && shift
rust_bin="${GITPROMPT_BIN:-$repo/backend/target/release/gitprompt}"

[[ -f "$db" ]] || { echo "no database at $db" >&2; exit 2; }
[[ -x "$rust_bin" ]] || { echo "no Rust binary at $rust_bin: run \`just build\`" >&2; exit 2; }

work="$(mktemp -d "${TMPDIR:-/tmp}/gitprompt-compare.XXXXXX")"
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT INT TERM

free_port() { python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'; }

echo "snapshot of $db"
mkdir -p "$work/python" "$work/rust"
python3 - "$db" "$work/python/gitprompt.db" <<'PY'
import sqlite3, sys
source = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
target = sqlite3.connect(sys.argv[2])
source.backup(target)
target.close()
PY
cp "$work/python/gitprompt.db" "$work/backend/gitprompt.db"

python_port="$(free_port)"
rust_port="$(free_port)"
GITPROMPT_HOME="$work/python" GITPROMPT_PORT="$python_port" \
  uv run --quiet --project "$repo/legacy/python" gitprompt --legacy serve >"$work/python.log" 2>&1 &
pids+=($!)
GITPROMPT_HOME="$work/rust" GITPROMPT_PORT="$rust_port" "$rust_bin" serve >"$work/rust.log" 2>&1 &
pids+=($!)

wait_up() { # port, log
  for _ in $(seq 1 600); do
    curl -fs -m 1 "http://127.0.0.1:$1/healthz" >/dev/null 2>&1 && return 0
    sleep 0.2
  done
  echo "daemon on port $1 did not start; its log:" >&2
  cat "$2" >&2
  return 1
}
wait_up "$python_port" "$work/python.log"
wait_up "$rust_port" "$work/rust.log"

echo "A = legacy Python (port $python_port), B = Rust (port $rust_port)"
python3 "$repo/tools/compare_backends.py" "http://127.0.0.1:$python_port" "http://127.0.0.1:$rust_port" "$@"
