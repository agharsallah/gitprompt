#!/usr/bin/env bash
# The guards' own tests: each case feeds a hook input to guard.sh or guard-bash.sh and checks the
# decision (allow, ask, deny). Run by the quality gate whenever the hooks change: `bash .claude/hooks/test.sh`.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
failed=0

decision() {
	local out
	out="$(printf '%s' "$2" | bash "$here/$1")"
	if [ -z "$out" ]; then
		echo allow
	else
		jq -r '.hookSpecificOutput.permissionDecision // "allow"' <<<"$out"
	fi
}

edit() { jq -n --arg f "$repo/$1" --arg new "${2:-x}" '{tool_input: {file_path: $f, old_string: "a", new_string: $new}}'; }
shell() { jq -n --arg c "$1" '{tool_input: {command: $c}}'; }

expect() {
	local want="$1" hook="$2" input="$3" label="$4" got
	got="$(decision "$hook" "$input")"
	if [ "$got" != "$want" ]; then
		echo "FAIL: $label: expected $want, got $got"
		failed=1
	fi
}

# Edits
expect deny guard.sh "$(edit legacy/python/gitprompt/cli.py)" 'legacy Python is frozen'
expect deny guard.sh "$(edit web/src/lib/api/generated.ts)" 'generated types'
expect ask guard.sh "$(edit backend/crates/gitprompt-core/tests/architecture/rules.rs)" 'architecture map'
expect ask guard.sh "$(edit backend/deny.toml)" 'dependency policy'
expect ask guard.sh "$(edit web/eslint.config.js)" 'eslint config'
expect ask guard.sh "$(edit web/vite.config.ts)" 'coverage thresholds'
expect ask guard.sh "$(edit backend/crates/gitprompt-server/tests/snapshots/api__prompts.snap)" 'API snapshots'
expect ask guard.sh "$(edit .claude/hooks/guard.sh)" 'the hooks'
expect ask guard.sh "$(edit .claude/settings.json)" 'hook settings'
expect ask guard.sh "$(edit .github/workflows/ci.yml)" 'CI'
expect ask guard.sh "$(edit backend/Cargo.toml 'unwrap_used = "allow"')" 'a workspace lint level'
expect allow guard.sh "$(edit backend/Cargo.toml 'serde = "1"')" 'a dependency in the workspace manifest'
expect allow guard.sh "$(edit backend/crates/gitprompt-core/src/json.rs)" 'ordinary Rust code'
expect allow guard.sh "$(edit web/src/lib/shared/format.ts)" 'ordinary web code'

# Shell commands
expect ask guard-bash.sh "$(shell "sed -i '' 's/deny/warn/' backend/deny.toml")" 'sed -i on deny.toml'
expect ask guard-bash.sh "$(shell 'cd backend && sed -i "" s/x/y/ deny.toml')" 'sed -i on deny.toml after cd'
expect ask guard-bash.sh "$(shell 'echo x > web/vite.config.ts')" 'redirect into vite config'
expect ask guard-bash.sh "$(shell 'python3 - <<EOF
open("web/eslint.config.js","w")
EOF')" 'a script writing the eslint config'
expect deny guard-bash.sh "$(shell 'cp x.py legacy/python/gitprompt/x.py')" 'copy into legacy'
expect ask guard-bash.sh "$(shell 'git commit --no-verify -m x')" 'skipping git hooks'
expect allow guard-bash.sh "$(shell 'cat backend/deny.toml')" 'reading deny.toml'
expect allow guard-bash.sh "$(shell 'grep -n skip backend/deny.toml 2>&1 | head')" 'grepping with a redirect of stderr'
expect allow guard-bash.sh "$(shell 'cargo test -q -p gitprompt-core > /dev/null')" 'output to /dev/null'
expect allow guard-bash.sh "$(shell 'just lint')" 'running the checks'

[ "$failed" = 0 ] && echo "hooks: every guard case passes"
exit "$failed"
