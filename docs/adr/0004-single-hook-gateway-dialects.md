# 0004 — One hook gateway, dialects as data

**Status:** superseded by [0008](0008-universal-capture-declarative-sources.md)

## Context
Claude Code, Copilot (CLI + VS Code), Gemini CLI, Cursor and Antigravity all have hooks. Their payloads differ mainly in field names (`session_id` vs `sessionId` vs `conversation_id`), and most follow Claude Code's shape. Some tools load *other* tools' hook files (Copilot reads `.claude/settings*.json`, Cursor loads Claude Code hooks), so a hook can fire from an unexpected caller.

## Decision
- A single endpoint `POST /v1/hooks/{connector}` plus a single stdlib-only shim `gp-hook <connector>` for tools that need a command.
- Payload differences are expressed as **`Dialect` mapping tables** (data), consumed by one generic mapper. Known dialects: `claude`, `copilot_camel`, `cursor`, `nudge`.
- The caller is identified from the **payload fingerprint**, not the install location.
- Native HTTP hooks are used where supported (Claude Code). The shim is fire-and-forget (≤300 ms), spools on failure, prints the dialect's no-op response and always exits 0.
- Only prompt-submit (and session-start) events are registered. Never blocking tool events.

## Consequences
+ Adding a hook-capable tool costs a dialect row + an installer template + a contract test.
+ Hooks can never block or break the user's tool.
− Dialect detection must be maintained as tools evolve → contract tests built from documented sample payloads.
