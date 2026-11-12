#!/usr/bin/env bash
# PostToolUse (Edit, Write, MultiEdit): formats a Rust file of the workspace right after Claude
# writes it, with the workspace's rustfmt.toml, so formatting never reaches the Stop gate or CI.
# A file that does not parse yet (an edit in progress) is left alone: the Stop gate reports it.
set -uo pipefail
here="$(dirname "$0")"
# shellcheck source=SCRIPTDIR/lib.sh
source "$here/lib.sh"

file="$(jq -r '.tool_response.filePath // .tool_input.file_path // empty' <<<"$(hook_input)")"
case "$(relative "$file")" in
backend/*.rs) ;;
*) exit 0 ;;
esac
[ -f "$file" ] || exit 0
command -v rustfmt >/dev/null || exit 0
rustfmt --edition 2024 --config-path "$REPO/backend/rustfmt.toml" "$file" >/dev/null 2>&1 || true
exit 0
