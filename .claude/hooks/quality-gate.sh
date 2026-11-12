#!/usr/bin/env bash
# Stop and SubagentStop: before Claude (or a subagent) finishes, run the checks for every part of the
# repository its working tree changed, however the change was made (an edit, a shell command, a
# script). Each check is a script in checks/; it runs when files under its paths changed, and is
# skipped when exactly these changes already passed it. A failure goes back to Claude (exit 2) with
# the output, so it fixes the cause before finishing. If it still fails on the next try, the user
# gets a warning instead of a loop; CI remains the final gate.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=SCRIPTDIR/lib.sh
source "$here/lib.sh"

# Where the gate remembers which change sets already passed (git-ignored).
GATE_STATE="$REPO/target/quality/.gate"
input="$(hook_input)"
retrying="$(jq -r '.stop_hook_active // false' <<<"$input")"
mkdir -p "$GATE_STATE"
cd "$REPO" || exit 0

failures=""
for check in "$here"/checks/*.sh; do
	name="$(basename "$check" .sh)"
	# Each check defines check_paths (its git pathspecs) and check_run (files changed, one per line).
	# shellcheck source=/dev/null
	result="$(
		source "$check"
		read -ra CHECK_PATHS <<<"$(check_paths)"
		files="$(git status --porcelain=v1 -uall -- "${CHECK_PATHS[@]}" 2>/dev/null | sed -E 's/^...//; s/^.* -> //')"
		[ -n "$files" ] || exit 0
		print="$(fingerprint "${CHECK_PATHS[@]}")"
		[ "$(cat "$GATE_STATE/$name" 2>/dev/null)" = "$print" ] && exit 0
		if output="$(check_run "$files" 2>&1)"; then
			printf '%s' "$print" >"$GATE_STATE/$name"
		else
			printf '%s\n' "$output" | grep -v -e '^\s*$' | tail -40
			exit 1
		fi
	)" || failures+=$'\n'"## ${name}"$'\n'"${result}"$'\n'
done

[ -z "$failures" ] && exit 0
if [ "$retrying" = "true" ]; then
	jq -n --arg failures "$failures" '{systemMessage: ("Quality gate still failing after a retry; CI will reject this:" + $failures)}'
	exit 0
fi
{
	echo "Quality gate: these checks fail for the changes in the working tree. Fix the cause (never"
	echo "silence a lint, lower a threshold or widen a guardrail to pass), then finish."
	printf '%s\n' "$failures"
} >&2
exit 2
