# 0002 — Hooks are the signal, transcripts are the truth

**Status:** amended by [0008](0008-universal-capture-declarative-sources.md): hooks only by default, transcripts become an optional add-on

## Context
Most tools offer prompt-submit hooks (instant, documented, but some are preview/experimental, and they only fire while the tool runs with the hook installed). All tools also write local transcripts (complete, include history, but undocumented and subject to change).

## Decision
Every connector uses **both** channels when available:
- The **hook** produces an immediate prompt record *and*, when it carries `transcript_path`, triggers an immediate read of that transcript.
- The **transcript reader** (watcher + periodic sweep + initial backfill) is the authoritative and complete source. It enriches hook-created records and recovers anything missed.
- Records from both channels are merged by deterministic merge keys (architecture §7.2).

## Consequences
+ Real-time capture with recovery: daemon downtime, hook failures, or a disabled hook lose nothing that the tool itself persisted.
+ When a tool update breaks one channel, the other keeps working while we fix the parser.
− Merge logic is required, and it's the most delicate code in the system → golden tests per connector.

## Alternatives considered
- **Hooks only:** no history, and anything missed is lost forever.
- **Transcripts only:** no real-time view, and full exposure to format drift.
- **OpenTelemetry export** (Claude Code `OTEL_LOG_USER_PROMPTS`, Gemini CLI telemetry): a viable third channel, but it requires env/config changes per tool and lacks some metadata. Revisit as an optional OTLP receiver later.
