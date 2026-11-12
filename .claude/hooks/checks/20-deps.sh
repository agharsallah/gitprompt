#!/usr/bin/env bash
# Rust dependencies, when a manifest, the lock file or the policy changed: `just deps` (cargo-deny:
# advisories, licenses, duplicates, sources; cargo-machete: unused crates).
# Sourced by quality-gate.sh: check_paths and check_run.

# Git pathspecs (a * crosses folders).
check_paths() { echo 'backend/*Cargo.toml backend/Cargo.lock backend/deny.toml'; }

check_run() {
	cd "$REPO" || return 1
	just deps || {
		echo "Dependencies: fix the cause (bump, remove, or ask the user before adding a skip to deny.toml)."
		return 1
	}
}
