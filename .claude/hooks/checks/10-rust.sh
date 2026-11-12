#!/usr/bin/env bash
# Rust: formatting, the architecture rules, clippy (warnings are errors) and the tests of every crate
# that changed (the whole workspace when shared config changed), and the generated TypeScript types
# when a wire type changed. What `just lint` and `just test` check for those crates.
# Sourced by quality-gate.sh: check_paths and check_run.

# Git pathspecs (a * crosses folders).
check_paths() { echo 'backend/*.rs backend/*Cargo.toml backend/Cargo.lock backend/clippy.toml backend/rustfmt.toml'; }

# `-p crate` for each changed crate, or --workspace when workspace-wide files changed.
packages() {
	if grep -qE '^backend/(Cargo\.(toml|lock)|clippy\.toml|rustfmt\.toml)$' <<<"$1"; then
		echo --workspace
		return
	fi
	sed -nE 's#^backend/crates/([^/]+)/.*#-p \1#p; s#^backend/apps/desktop/.*#-p gitprompt-desktop#p' <<<"$1" | sort -u | tr '\n' ' '
}

check_run() {
	local files="$1" pkgs=()
	read -ra pkgs <<<"$(packages "$files")"
	cd "$REPO/backend" || return 1
	cargo fmt --all --check || {
		echo "Formatting: run \`just fmt\`."
		return 1
	}
	cargo test -q -p gitprompt-core --test architecture || {
		echo "Architecture rules: fix the cause (docs/extending.md, Architecture); the map is not yours to widen."
		return 1
	}
	[ "${#pkgs[@]}" -gt 0 ] || return 0
	cargo clippy -q "${pkgs[@]}" --all-targets -- -D warnings || return 1
	cargo test -q "${pkgs[@]}" || return 1
	# A changed wire type (derives ts-rs) must be regenerated into web/src/lib/api/generated.ts.
	local rust=()
	while IFS= read -r f; do [ -f "$REPO/$f" ] && rust+=("$REPO/$f"); done < <(grep -E '\.rs$' <<<"$files")
	if [ "${#rust[@]}" -gt 0 ] && grep -lE 'derive\([^)]*\bTS\b|#\[ts\(' "${rust[@]}" >/dev/null 2>&1; then
		cargo test -q -p gitprompt-server --test ts_types || {
			echo "generated.ts is stale: run \`just types\` and commit both (CLAUDE.md)."
			return 1
		}
	fi
}
