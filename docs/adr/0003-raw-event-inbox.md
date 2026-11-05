# 0003 — Append-only raw event inbox, replayable normalization

**Status:** amended by [0008](0008-universal-capture-declarative-sources.md): iteration 1 keeps the raw payload in the prompt row and rebuilds fields with `remap`

## Context
Parsers will be wrong or outdated at times (undocumented formats). Re-reading source files later isn't always possible: tools prune history, and hook payloads exist only once.

## Decision
- Ingestion writes the verbatim payload to `raw_event` (unique on `connector, channel, native_key`) and acknowledges.
- A worker normalizes unprocessed events into `prompt`/`session`/`project`, recording `parser_version` and `parse_status`.
- `gitprompt reprocess --connector X [--since …]` replays normalization after a parser fix.

## Consequences
+ Ingestion is fast, idempotent and never loses data. Hooks return within ~50 ms.
+ Parser bugs are recoverable. Provenance links every prompt to its raw events.
− Storage duplication → configurable retention (and optional encryption) for raw payloads.
