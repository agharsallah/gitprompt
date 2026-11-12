#!/usr/bin/env bash
# The tooling around the code: the quality dashboard's scripts parse (tools/quality), shell scripts
# pass shellcheck, and the hooks still guard what they should (.claude/hooks/test.sh).
# Sourced by quality-gate.sh: check_paths and check_run.

# Git pathspecs (a * crosses folders).
check_paths() { echo 'tools/* .claude/hooks/* .claude/settings.json'; }

check_run() {
	local files="$1" ok=0
	cd "$REPO" || return 1
	while IFS= read -r f; do
		[ -f "$f" ] || continue
		case "$f" in
		*.mjs | *.js) node --check "$f" || ok=1 ;;
		*.sh) if command -v shellcheck >/dev/null; then shellcheck -x "$f" || ok=1; fi ;;
		esac
	done <<<"$files"
	if grep -qE '^\.claude/(hooks/|settings\.json)' <<<"$files"; then
		jq empty .claude/settings.json || ok=1
		bash .claude/hooks/test.sh || ok=1
	fi
	return "$ok"
}
