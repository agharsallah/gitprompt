# 0011 — Drop telemetry lifecycle noise; `gitprompt compact` and event retention

**Status:** accepted (implemented)
**Amends:** [0009](0009-metadata-events-telemetry-derived-on-read.md) (events are still all kept by default, except the ones a spec lists as noise)

## Context
After five days of real use the database was 359 MB. About 40% of it (114k of 166k event rows, 142 MB of stored JSON) came from Claude Code telemetry events that fire at every start-up or around every hook run: `hook_registered`, `plugin_loaded`, `hook_execution_start/complete`, `mcp_server_connection`, `managed_settings_resolved` and `retention_sweep`. No derivation, page or catalog entry reads them. The only generic consumer is the session's "events by type" count.

The spec already had `accept_events` (an allowlist, empty = keep all). An allowlist would silently drop every useful event a future Claude Code version adds.

## Decision
1. **A `drop_events` map in the source spec**: event name -> why it is not stored. A trailing `*` matches a prefix (`hook_execution_*`). `Spec.keeps(event)` checks `accept_events` first, then `drop_events`. The ingest pipeline calls it for every source, so a source needs no special code. Events that are not listed are still kept.
2. **The reasons appear on the Fields page** next to `not_captured`, so the user sees what is received but not stored, and why.
3. **`gitprompt compact`** (`capture/compact.py`, with the CLI as a thin wrapper) deletes stored events the *current* specs would not keep, then runs `VACUUM`. It is a dry run unless `--apply` is given, and the dry run prints rows and approximate bytes per event. Events of a source with no spec are left alone.
4. **Retention is opt-in**: `--events-older-than DAYS` also deletes raw `event` rows older than that. It never deletes `prompt` or `annotation` rows. `compact.expired` / `compact.compact` are plain functions, so the Settings page can expose a persistent retention setting later without new storage code.

## Consequences
+ On a copy of the real DB: 359 MB -> 159 MB after `compact --apply`. About 56% of daily growth was noise, and it is no longer written.
+ A tool update that adds noise is handled by one YAML line (or a user override in `~/.gitprompt/sources/`) plus `compact`.
− The session "events by type" count no longer includes the dropped kinds.
− Expired events take turn details with them (tools, usage, timeline) for prompts older than the cutoff. That is why retention is off by default.
− Unlike remapping, compaction cannot be undone: removing a name from `drop_events` keeps future events, not deleted ones.

## Alternatives considered
- **Exhaustive `accept_events` allowlist:** would silently lose new useful events after a tool update.
- **Turning these events off in Claude Code:** the monitoring docs list no per-event switch for OTel log events, and the receiver has to answer every batch anyway.
- **Deleting noise automatically at start-up:** destructive without consent. Kept as an explicit command that defaults to a dry run.
