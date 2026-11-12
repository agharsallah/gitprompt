#!/usr/bin/env bash
# The web dashboard: Prettier and ESLint on the changed files (also those changed by a shell command,
# which the per-edit hook never saw), svelte-check, and the unit tests with their coverage
# thresholds. What `just lint` and `just test` check for web/.
# Sourced by quality-gate.sh: check_paths and check_run.

# Git pathspecs (a * crosses folders).
check_paths() { echo 'web/*'; }

check_run() {
	# Changed files that still exist, relative to web/; the code among them.
	local files=() code=()
	while IFS= read -r f; do
		[ -f "$REPO/$f" ] || continue
		files+=("${f#web/}")
		case "$f" in *.ts | *.js | *.svelte) code+=("${f#web/}") ;; esac
	done <<<"$1"
	cd "$REPO/web" || return 1
	if [ "${#files[@]}" -gt 0 ]; then
		pnpm exec prettier --check --ignore-unknown --log-level warn "${files[@]}" || {
			echo "Formatting: run \`just fmt\`."
			return 1
		}
	fi
	if [ "${#code[@]}" -gt 0 ]; then pnpm exec eslint --no-warn-ignored "${code[@]}" || return 1; fi
	pnpm check >/dev/null 2>&1 || {
		pnpm check 2>&1 | grep -E 'Error|error' | head -20
		echo "Type errors (svelte-check): fix them with real types, never \`any\` or @ts-ignore."
		return 1
	}
	pnpm test:coverage || {
		echo "Tests or coverage thresholds (vite.config.ts) fail: fix the test or add tests; never lower a threshold."
		return 1
	}
}
