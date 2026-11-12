#!/usr/bin/env bash
# PreToolUse (Edit, Write, MultiEdit, NotebookEdit): refuses frozen and generated files, and asks
# the user before a guardrail changes (protected.sh). Also asks before lint levels in a Cargo
# manifest change. Anything else passes untouched.
set -uo pipefail
here="$(dirname "$0")"
# shellcheck source=SCRIPTDIR/lib.sh
source "$here/lib.sh"
# shellcheck source=SCRIPTDIR/protected.sh
source "$here/protected.sh"

input="$(hook_input)"
file="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")"
[ -n "$file" ] || exit 0
path="$(relative "$file")"

rule="$(protection "$path")"
case "$rule" in
deny\|*) refuse "${rule#deny|}" ;;
ask\|*) ask "${rule#ask|}" ;;
esac

case "$path" in
backend/Cargo.toml | backend/crates/*/Cargo.toml | backend/apps/*/Cargo.toml)
	text="$(jq -r '[.tool_input.old_string, .tool_input.new_string, .tool_input.content, (.tool_input.edits // [] | map(.old_string, .new_string))] | flatten | map(select(. != null)) | join("\n")' <<<"$input")"
	if lint_section_edit "$text"; then
		ask 'This changes lint levels in a Cargo manifest (the workspace lints are a hard CI gate). Approve only a deliberate policy change, never to make a warning go away.'
	fi
	;;
esac
exit 0
