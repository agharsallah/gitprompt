#!/usr/bin/env bash
# PreToolUse hook: the architecture map (crate layers, owned infrastructure crates, pure crates) is
# changed on purpose, never to make a check pass. Any edit to it asks the user first.
set -uo pipefail

file=$(jq -r '.tool_input.file_path // empty')
case "$file" in
	*/gitprompt-core/tests/architecture/rules.rs)
		jq -n '{hookSpecificOutput: {
			hookEventName: "PreToolUse",
			permissionDecision: "ask",
			permissionDecisionReason: "This changes the architecture map (which crate may use which, owned infrastructure crates, pure crates). Approve only a design change you agreed to, not a way around a failing check."
		}}'
		;;
esac
exit 0
