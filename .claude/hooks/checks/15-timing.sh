#!/usr/bin/env bash
# Time dependencies, when Rust or dashboard code, or the check itself, changed: `just timing`
# (tools/timing). A new blocking one (a sleep, raw timeout or timing assertion in a test, a clock
# read in code) the ledger does not accept fails; timers and stopwatches are only reported.
# Sourced by quality-gate.sh: check_paths and check_run.

# Git pathspecs (a * crosses folders).
check_paths() { echo 'backend/crates/* backend/apps/* web/src/* tools/timing/*'; }

check_run() {
	cd "$REPO" || return 1
	node tools/timing/cli.mjs || {
		echo "Time dependencies: wait for the event instead of a time, inject the clock, assert outcomes not durations (each rule says how); only the user accepts a new line in tools/timing/ledger.json."
		return 1
	}
}
