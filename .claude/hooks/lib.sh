#!/usr/bin/env bash
# Shared by the hooks in this folder. Sourced, not run.

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# The hook's JSON input, read once.
hook_input() {
	if [ -z "${HOOK_INPUT+set}" ]; then HOOK_INPUT="$(cat)"; fi
	printf '%s' "$HOOK_INPUT"
}

# A path relative to the repository (absolute paths from tools come in as is).
relative() {
	case "$1" in
	"$REPO"/*) printf '%s' "${1#"$REPO"/}" ;;
	*) printf '%s' "$1" ;;
	esac
}

# Files changed in the working tree (tracked and untracked), relative to the repository, one per line.
changed_files() {
	git -C "$REPO" status --porcelain=v1 -uall 2>/dev/null | sed -E 's/^...//; s/^.* -> //; s/^"(.*)"$/\1/'
}

# A fingerprint of the current content of the changed files matching the git pathspecs given.
fingerprint() {
	{
		git -C "$REPO" diff HEAD -- "$@" 2>/dev/null
		git -C "$REPO" ls-files --others --exclude-standard -- "$@" 2>/dev/null | while IFS= read -r file; do
			printf '%s\n' "$file"
			shasum "$REPO/$file" 2>/dev/null
		done
	} | shasum | cut -c1-40
}

# PreToolUse answers: let the user decide, or refuse with the reason going back to Claude.
ask() {
	jq -n --arg reason "$1" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "ask", permissionDecisionReason: $reason}}'
	exit 0
}
refuse() {
	jq -n --arg reason "$1" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
	exit 0
}
