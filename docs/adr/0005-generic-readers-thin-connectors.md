# 0005 — Generic readers, thin declarative connectors

**Status:** superseded by [0008](0008-universal-capture-declarative-sources.md) (readers may return in the optional history add-on)

## Context
Tools store history in only a few physical shapes: append-only JSONL (Claude Code, Copilot CLI, Antigravity, Codex), JSONL mutation logs (VS Code chat), whole JSON files (Gemini, legacy VS Code), and SQLite (Cursor, Cursor CLI, Copilot session store, Antigravity IDE). Incremental reading, checkpointing and edge cases (truncation, rotation, partial lines, WAL, locks) are the hard parts, and they're identical across tools.

## Decision
- `gitprompt-readers` implements `jsonl_tail`, `json_snapshot`, `sqlite_snapshot`, `json_patch_log`, plus OS/VS Code-family discovery, each with its own checkpoint type and property-based tests.
- A connector is a **manifest** (detect paths, channels, installer templates, tested versions, privacy notes) plus **pure parse functions** `(record, ctx) -> Iterable[RawEvent]`. Connectors never open files or talk to the DB.
- Connectors are registered via the Python entry point group `gitprompt.connectors`.

## Consequences
+ The hard IO code is written and tested once. New connectors are ~100–250 LOC + fixtures.
+ Parse functions are trivially golden-testable from fixtures.
− An exotic format needs a new reader, which is acceptable and reusable.
