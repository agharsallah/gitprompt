#!/usr/bin/env bash
# PreToolUse (Bash): the same protection as guard.sh for edits made through a shell command (sed -i,
# redirections, tee, mv, cp, rm, a script writing files), and a check on commands that skip
# verification. Reading a protected file is always fine.
set -uo pipefail
here="$(dirname "$0")"
# shellcheck source=SCRIPTDIR/lib.sh
source "$here/lib.sh"
# shellcheck source=SCRIPTDIR/protected.sh
source "$here/protected.sh"

command="$(jq -r '.tool_input.command // empty' <<<"$(hook_input)")"
[ -n "$command" ] || exit 0

if grep -qE -- '--no-verify|--no-gpg-sign' <<<"$command"; then
	ask 'This command skips verification (git hooks or signing). Approve only if you meant to bypass them.'
fi

# Does the command write files? Redirections to /dev/null and between descriptors don't count.
writes="$(sed -E 's#[0-9]*>&[0-9]##g; s#&?>>?[[:space:]]*/dev/null##g' <<<"$command")"
grep -qE '(^|[^<>])>>?|sed[[:space:]].*-i|\btee\b|\b(mv|cp|rm|truncate|install)\b|\bperl\b.*-[a-z]*i|python3?\b|\bnode\b.*-e|git (checkout|restore)\b' <<<"$writes" || exit 0

# Every word that looks like a path, checked against the protected table.
# A command may `cd backend` first and name `deny.toml` alone: try a bare name in each part too.
for word in $(tr -s ' \t;|&<>()"'"'"'=' '\n' <<<"$command" | grep -E '[/.]|^justfile$'); do
	path="$(relative "${word#./}")"
	for candidate in "$path" "backend/$path" "web/$path"; do
		rule="$(protection "$candidate")"
		case "$rule" in
		deny\|*) refuse "${rule#deny|} (via a shell command touching ${word})" ;;
		ask\|*) ask "${rule#ask|} (a shell command that may write ${word})" ;;
		esac
	done
done
exit 0
