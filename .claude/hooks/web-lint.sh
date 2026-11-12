#!/usr/bin/env bash
# PostToolUse hook: format and lint a dashboard file (web/) right after Claude writes it.
# Prettier and `eslint --fix` repair what they can; anything ESLint still reports goes back to
# Claude (exit 2 + stderr) so it fixes the code before moving on. Other files are left alone.
set -uo pipefail

file=$(jq -r '.tool_response.filePath // .tool_input.file_path // empty')
web="$(cd "$(dirname "$0")/../../web" && pwd)"

case "$file" in
	"$web"/*) ;;
	*) exit 0 ;;
esac
[ -f "$file" ] || exit 0
cd "$web" || exit 0

# Prettier skips files it has no parser for and honours .prettierignore.
node_modules/.bin/prettier --write --ignore-unknown --log-level warn "$file" >&2

case "$file" in
	*.ts | *.js | *.svelte) ;;
	*) exit 0 ;;
esac

if ! out=$(node_modules/.bin/eslint --fix --no-warn-ignored "$file" 2>&1); then
	echo "ESLint still reports problems in ${file#"$web"/} (auto-fixes applied):" >&2
	echo "$out" >&2
	exit 2
fi
