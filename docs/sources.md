# Prompt sources — research notes

How each tool stores and exposes prompts, from two kinds of evidence:
- **L (local):** structure inspected on the author's macOS machine on 2026-11-01 (keys/schemas only, no prompt content). Tool versions are noted.
- **D (docs):** official documentation, linked under References.

Items marked **?** are open and assigned to Phase 0. When a tool update changes a format, update this file and add a fixture for the new version.

---

## Claude Code — CLI 2.1.316 (also used by the VS Code / Cursor extension)
**Live (D):** hooks in `~/.claude/settings.json`. `UserPromptSubmit` gets common fields `session_id`, `prompt_id`, `transcript_path`, `cwd`, `permission_mode`, `effort`, `hook_event_name`, plus `prompt`. Supports `type: "http"` hooks (POST JSON, non-2xx = non-blocking error) and `async: true` command hooks. Default timeout on `UserPromptSubmit`: 30 s.
**Backfill (L):**
- `~/.claude/projects/<cwd-slug>/<sessionId>.jsonl`. Record types include `user`, `assistant`, `attachment`, `system`, `ai-title`, `last-prompt`, `mode`, `file-history-*`.
- `user` record: `promptId`, `uuid`, `parentUuid`, `isSidechain`, `timestamp`, `sessionId`, `cwd`, `gitBranch`, `version`, `entrypoint` (CLI vs IDE), `permissionMode`, `origin.kind`, `promptSource`, `turnPosition`, `message.content` (string, or array of content blocks).
- `assistant` record: `message.model`, `requestedModel`, `effort`, `usage` → the model for the turn comes from the next assistant record.
- `~/.claude/history.jsonl`: `display`, `pastedContents`, `timestamp`, `project`, `sessionId`. This is the prompt-input history, good for quick backfill.

**Join key:** hook `prompt_id` == transcript `promptId`.

**Verified 2026-11-02 (D + L, Claude Code 2.1.316), used by iteration 1:**
- `UserPromptSubmit` payload: `session_id`, `prompt_id`, `transcript_path`, `cwd`, `permission_mode`, `hook_event_name`, `prompt` (pasted `[Pasted text #N]` placeholders arrive expanded), and `session_title` when the session has a custom title. A live run confirmed all of these except `session_title`.
- HTTP hook: `{type: "http", url, timeout, headers, allowedEnvVars, onFailure}`. Header values may interpolate `$VAR` only for variables listed in `allowedEnvVars`; GitPrompt uses a literal header. Response handling: **2xx with an empty body = success with no output**; non-2xx, connection failure and timeout are non-blocking errors unless `onFailure: "block"` (never set by GitPrompt). A live run with a wrong token confirmed the prompt still went through.
- Output that would affect the prompt (`decision: "block"`, `additionalContext`, `sessionTitle`, plain-text stdout for command hooks) is never produced by GitPrompt.
- Settings files are watched: hook edits apply to running sessions without a restart. `CLAUDE_CONFIG_DIR` relocates `~/.claude`.
- Policy keys: `disableAllHooks` (any level), `allowManagedHooksOnly` (managed: blocks user hooks), `allowedHttpHookUrls` / `httpHookAllowedEnvVars` (URL/env allowlists, merged across files). Managed files: `/Library/Application Support/ClaudeCode/` (macOS), `/etc/claude-code/` (Linux/WSL), `C:\Program Files\ClaudeCode\` (Windows), each with `managed-settings.json` + optional `managed-settings.d/`. Server-managed and MDM policies can't be read from disk.
- Settings files are strict JSON (comments are a Settings Error), so the installer refuses to write a file it can't parse.

**Verified 2026-11-03 (D + L, Claude Code 2.1.317), used by iteration 1b:**
- HTTP hooks run on every event except `SessionStart` and `Setup` ("Only `type: \"command\"` and `type: \"mcp_tool\"` hooks are supported"). The common fields also include `scratchpad_dir`, `effort.level`, `agent_id` and `agent_type`. `Stop` carries `last_assistant_message`, and `PostToolUse` carries `tool_input`, `tool_response`, `tool_use_id` and `duration_ms`. The full per-event list is in `src/gitprompt/sources/bundled/claude-code.yaml` (`documented`).
- `strictPluginOnlyCustomization` (`true` or a list containing `"hooks"`, managed only) also blocks user hooks.
- OpenTelemetry: `CLAUDE_CODE_ENABLE_TELEMETRY=1` and `OTEL_{LOGS,METRICS}_EXPORTER=otlp` with `OTEL_EXPORTER_OTLP_PROTOCOL=http/json` are accepted in user `settings.json` `env` (and ignored in project settings).
  - Precedence: the launch environment wins over settings files, and a managed endpoint or headers remove developer-set endpoints.
  - Events carry `prompt.id`, which equals the hooks' `prompt_id` (confirmed live), and `session.id`, which equals `session_id`.
  - Metric temporality defaults to delta, so session totals are sums.
- A live run produced these telemetry types: `user_prompt`, `assistant_response`, `api_request`, `tool_decision`, `tool_result`, `hook_execution_start/complete`, `hook_registered`, `plugin_loaded`, `mcp_server_connection`, `managed_settings_resolved`, and the metrics `session.count`, `lines_of_code.count`, `commit.count`, `cost.usage`, `token.usage`, `code_edit_tool.decision` and `active_time.total`. Telemetry also attaches `user.email`, `organization.id` and account IDs (stored locally only).

## GitHub Copilot CLI — 1.0.102
**Live (D):** user hooks in `~/.copilot/hooks/*.json` (or `$COPILOT_HOME/hooks/`), or inline `hooks` in `~/.copilot/settings.json`. `config.json` is **no longer read** for hooks. Repo-level `.github/hooks/*.json`, and it also reads `.claude/settings*.json`. Event `userPromptSubmitted` (camelCase payload: `sessionId`, `timestamp` ms, `cwd`, `prompt`), or the PascalCase `UserPromptSubmit` (snake_case: `hook_event_name`, `session_id`, ISO `timestamp`, `cwd`, `prompt`). `sessionStart` carries `source` and an optional `initialPrompt`. Default timeout 30 s. Non-zero exit is logged and skipped, timeouts fail open. Supports command **and HTTP** hooks.
**Backfill (L):**
- `~/.copilot/session-state/<sessionId>/events.jsonl`. `session.start.data` = `sessionId`, `copilotVersion`, `producer`, `startTime`, `context.{cwd, gitRoot, repository, branch, headCommit, hostType}`. `session.model_change.data.newModel`. `user.message.data` = `content` (typed), `transformedContent` (expanded), `messageId`, `interactionId`, `turnId`, `parentAgentTaskId` (sub-agent marker). `assistant.message.data.model`.
- `workspace.yaml` (session summary: cwd, git_root, branch, name, timestamps).
- `~/.copilot/session-store.db` (SQLite): `sessions`, `turns(user_message, assistant_response, turn_index)`, `session_files`, FTS index. A good secondary source.

**? Phase 0:** HTTP hook config shape; whether hook payloads let us tell CLI from VS Code (otherwise: look up `sessionId` in `session-state/`).

## GitHub Copilot Chat in VS Code — VS Code 1.144
**Live (D, preview):** VS Code's "Copilot harness" shares the hook format with Copilot CLI and loads `~/.copilot/hooks/`, `.github/hooks/*.json`, inline settings, and repo `.claude/settings*.json`. Event `userPromptSubmitted`. Local harness setting `chat.useHooks` (+ `chat.useClaudeHooks`). Organizations can disable hooks.
**Backfill (L):**
- `~/Library/Application Support/Code/User/workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl` — **mutation log**: line `kind:0` = full session snapshot (`sessionId`, `creationDate`, `requests[]`, `inputState.selectedModel`, `inputState.mode`), `kind:1` = set value at path `k`, `kind:2` = append/splice at path `k` (e.g. `["requests"]`).
- Request object: `requestId`, `timestamp`, `message.text`, `message.parts`, `variableData.variables` (attachments), `modelId`, `modeInfo.{modeId, modeName, kind}`, `agent.{id, extensionVersion}`, `response`, `result`.
- Legacy `chatSessions/*.json` (full snapshot: `requests[]`, `customTitle`, `creationDate`, `lastMessageDate`), still present on this machine (26 files) next to 64 `.jsonl`.
- `workspaceStorage/<hash>/workspace.json` → `{folder: "file:///…"}`. `globalStorage/emptyWindowChatSessions/` covers chats with no folder open.

Same layout under `Code - Insiders`, and potentially other VS Code forks (shared resolver).

## Gemini CLI — 0.33.2
**Live (D):** hooks in `~/.gemini/settings.json` (project `.gemini/settings.json` takes priority). Common input: `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `timestamp`. `BeforeAgent` adds `prompt`. Config: `hooks.<Event>[] = {matcher?, sequential?, hooks:[{type:"command", command, name, timeout(ms)}]}`. Default timeout 60 s. Exit 2 blocks, other non-zero codes are warnings. **stdout must be only JSON.** Hooks were introduced in v0.26.0. Older docs mention enable flags (`tools.enableHooks`, `hooks.enabled`) → **?** check whether 0.33 still needs them.
**Backfill (L):** `~/.gemini/tmp/<project-name>/logs.json` = array of `{sessionId, messageId, type:"user", message, timestamp}` (user prompts only). `chats/` holds saved/auto-saved sessions (empty on this machine). `~/.gemini/projects.json` maps project paths to tmp names.

## Antigravity — IDE 1.107 (`~/.gemini/antigravity-ide`) and app (`~/.gemini/antigravity`)
**Live (D):** hooks in `~/.gemini/config/hooks.json` (global) or `.agents/hooks.json` (workspace). Events: `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`. Common fields: `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName`. `PreInvocation` adds `invocationNum` and `initialNumSteps`. **No prompt text field is documented** → use as a nudge to read the transcript. Default timeout 30 s.
**Backfill (L):**
- `brain/<conversationId>/.system_generated/logs/transcript.jsonl` (also `transcript_full.jsonl`): records `{type, content, created_at, source, status, step_index, tool_calls?, thinking?}`. Types include `USER_INPUT`, `PLANNER_RESPONSE`, `CODE_ACTION`, `VIEW_FILE`, `RUN_COMMAND`, `SYSTEM_MESSAGE`, `CONVERSATION_HISTORY`.
- IDE `conversations/<id>.db` (SQLite, tables `steps`, `trajectory_meta`, `gen_metadata`, …, with protobuf payloads). Plain protobuf (decodable schemaless), a fallback only.
- App `conversations/<id>.pb`: **encrypted** (entropy 8.0). Not used.
- VS Code-fork state: `~/Library/Application Support/Antigravity IDE/User/globalStorage/state.vscdb` (`antigravityUnifiedStateSync.trajectorySummaries`, `chat.ChatSessionStore.index`) — candidate for conversation titles and workspace mapping.
- Docs also mention an Antigravity CLI at `~/.gemini/antigravity-cli` (not installed here).

## Cursor — IDE (Cursor.app)
**Live (D):** `~/.cursor/hooks.json` (`{version:1, hooks:{<event>:[{command, timeout, matcher?, failClosed?}]}}`), plus project/team/enterprise levels. Common input: `conversation_id`, `generation_id`, `model`, `model_id?`, `hook_event_name`, `cursor_version`, `workspace_roots`, `user_email`, `transcript_path` (null if transcripts are disabled). `beforeSubmitPrompt` adds `prompt` and `attachments[{type: file|rule, file_path}]`. Cursor can also load third-party (Claude Code) hooks. Community reports describe some `beforeSubmitPrompt` quirks (output fields ignored) — irrelevant to us, since we only observe.
**Backfill (L):** `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`:
- `cursorDiskKV`: `composerData:<composerId>` (`name`, `createdAt`, `lastUpdatedAt`, `unifiedMode`, `forceMode`, `context.*`, `fullConversationHeadersOnly[]`), `bubbleId:<composerId>:<bubbleId>` (`type` 1 = user / 2 = AI, `text`, `richText`, `createdAt`, `isSimulatedMsg`, `serverBubbleId`, `unifiedMode`, `tokenCount`).
- `composerHeaders` table: `composerId, workspaceId, createdAt, lastUpdatedAt, isArchived, isSubagent, subagentTypeName`.
- Per-workspace `workspaceStorage/<id>/state.vscdb` + `workspace.json` for folder mapping.

**? Phase 0:** where the model per bubble lives. Map `generation_id` ↔ bubble id. Which file `transcript_path` points to (it may be a simpler backfill source). Whether `~/.claude/settings.json` hooks fire inside Cursor.

## Cursor CLI — `cursor-agent` 2026.10.22
**Live:** **?** CLI hook support isn't documented (only `workspaceOpen` is stated to run in the CLI).
**Backfill (L):** `~/.cursor/chats/<workspaceHash>/<agentId>/meta.json` (`cwd`, `createdAtMs`, `updatedAtMs`) + `store.db`: `meta` (hex-encoded JSON: `agentId`, `latestRootBlobId`, `name`, `mode`, `createdAt`, `blobEncryptionKey`) and `blobs(id sha256, data)` — a content-addressed tree. Some blobs are binary/encrypted. Messages appear as plain JSON blobs `{role, content, providerOptions}`. User content may embed large injected context → extract the typed part. Best effort.

## OpenAI Codex CLI (installed)
**Backfill (L):** `~/.codex/sessions/**/*.jsonl` with `session_meta`, `turn_context`, `event_msg`, `response_item`. **?** Codex hook support and user-message event shape.

## Web: Copilot (github.com/copilot), Gemini (gemini.google.com)
- No local storage we can rely on → browser extension with request interception (MAIN world), DOM fallback.
- Gemini history export: Google Takeout → *My Activity › Gemini Apps*.
- Copilot web export: **?** none known.
- Later candidates via official exports: ChatGPT (Settings → Export), claude.ai (Settings → Export data).

## Cross-cutting observations
- **Hook convergence:** Claude Code's format is the de-facto standard. Copilot accepts it (PascalCase events, reads `.claude/settings*.json`), Cursor loads Claude Code hooks, and Gemini CLI uses the same snake_case common fields.
- **Every tool writes a transcript, and most hooks pass its path**, so the hook is a good real-time trigger for the transcript reader even when the hook payload is thin.
- **Formats move:** VS Code moved `.json` → `.jsonl` mutation logs and Cursor moved composer headers into a table. Both old and new forms coexist on this machine, so parsers must handle several versions at once.

## References
- Claude Code hooks: https://code.claude.com/docs/en/hooks
- Claude Code settings and managed settings: https://code.claude.com/docs/en/settings, https://code.claude.com/docs/en/managed-settings
- Claude Code monitoring (OpenTelemetry, `OTEL_LOG_USER_PROMPTS`): https://docs.anthropic.com/en/docs/claude-code/monitoring-usage
- Copilot CLI hooks reference: https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference
- Copilot CLI hooks tutorial: https://docs.github.com/en/copilot/tutorials/copilot-cli-hooks
- VS Code agent hooks: https://code.visualstudio.com/docs/copilot/customization/hooks
- Gemini CLI hooks reference: https://geminicli.com/docs/hooks/reference/
- Cursor hooks: https://cursor.com/docs/agent/hooks
- Antigravity hooks: https://www.antigravity.google/docs/hooks?tab=ide
