#!/usr/bin/env bash
# Stop hook: before Claude ends a turn while Rust code or a Cargo manifest has uncommitted changes,
# run the architecture check (`just arch`). A finding goes back to Claude (exit 2) so it fixes the
# cause before finishing. It blocks once per stop (`stop_hook_active`); CI is the hard gate.
set -uo pipefail

input=$(cat)
[ "$(jq -r '.stop_hook_active // false' <<<"$input")" = "true" ] && exit 0

cd "$(dirname "$0")/../.." || exit 0
# Git pathspec `*` crosses folders: every .rs file and Cargo.toml under backend/.
[ -n "$(git status --porcelain -- 'backend/*.rs' 'backend/*Cargo.toml' 2>/dev/null)" ] || exit 0

if ! out=$(cd backend && cargo test -q -p gitprompt-core --test architecture 2>&1); then
	{
		echo "The architecture check failed (just arch). Fix the cause in the code; widening the map in"
		echo "backend/crates/gitprompt-core/tests/architecture/rules.rs is a design change to agree with the user first."
		echo
		grep -v -e '^note: run with' -e '^thread ' -e '^\s*$' <<<"$out" | tail -60
	} >&2
	exit 2
fi
