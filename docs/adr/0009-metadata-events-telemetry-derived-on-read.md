# 0009 — Rich metadata: all observational events, opt-in telemetry, derived on read

**Status:** accepted (implemented)
**Amends:** [0008](0008-universal-capture-declarative-sources.md) (adds OpenTelemetry as a separate opt-in source and events beyond the prompt)

## Context
Iteration 1 stored the prompt and the `UserPromptSubmit` payload. To decide which features to build, we want every piece of metadata we can get without breaking a tool's terms, needing new permissions, or sending anything off the machine. That includes what happened during the turn, tokens and cost, the git state and commits, and the user's own notes. Claude Code documents three relevant interfaces:
- hooks on many more events (tool calls, turn end, subagents, compaction, model switches, session end);
- an OpenTelemetry export, whose events carry `prompt.id`, the same value as the hooks' `prompt_id`;
- nothing else we're willing to use, since transcripts are a private format (ADR-0008).

## Decision
1. **Subscribe to every observational hook event that accepts HTTP hooks.** Skip:
   - decision points (`PreToolUse`, `PermissionRequest`, `PreModelSwitch`, `Elicitation*`, `TeammateIdle`);
   - events where a hook changes behaviour (`WorktreeCreate`);
   - events that don't accept HTTP hooks (`SessionStart`, `Setup`);
   - duplicates or very frequent events (`PostToolBatch`, `MessageDisplay`, `FileChanged`).

   Each skipped event and its reason is listed in the spec (`not_captured`) and shown on the Fields page. Every call still gets an empty 2xx.
2. **Telemetry is a separate source** (`claude-code-otel`) with its own consent step. It sets documented `env` values in the user settings and points the OTLP/HTTP JSON exporter at the local receiver.
   - It is blocked if an endpoint is already configured, by the organization or the user.
   - Values from the launch environment and from managed settings take precedence by Claude Code's own rules, so it can't redirect an existing pipeline.
   - Prompt text and raw API bodies are not enabled.
3. **Events go into one `event` table**, joined to prompts by `prompt_id` and `session_id`. Long strings, such as file contents in tool results, are clipped before storage.
4. **Everything else is derived on read** (`insights/derivations.py`, `core/git.py`): prompt stats, secret detection, turn summary, usage, session totals, and commits after the prompt. Improving a rule never needs a migration, and old prompts benefit too.
5. **Plain config values we add are recorded at connect time.** Disconnect removes them only if they are unchanged. List entries such as hooks are still recognised by the receiver URL. A connection made by an older spec is reported as outdated, and updating it replaces our entries instead of duplicating them.
6. **The user's own notes** (star, rating, tags, note) are stored separately from captured data. Detected secrets can be hidden in place.

## Consequences
+ Per prompt: response time and text, tools, files, shell commands, subagents, tokens, cost, models, API time, rejections and commits. Per session: end reason, loaded instructions, metric totals and environment.
+ A Fields catalog compares documented fields with what was actually seen.
− More data on disk, including the account email and organization id that telemetry attaches. It stays local, but the DB deserves the same care as the prompts.
− Telemetry applies only to sessions started after connecting, and arrives in 5-second batches. The timeline orders events by `event.timestamp` when it is present.
− The hook count per session grows (one localhost POST per tool call). Each call has a 2 s timeout and never blocks.

## Alternatives considered
- **Read transcripts for tokens and model:** a private format (ADR-0008). Telemetry covers the same data through a documented interface.
- **Put telemetry in the hooks spec:** one consent for two different kinds of change. Kept separate.
- **Compute summaries at ingest:** faster lists, but every rule change would need a backfill. Kept on read.
