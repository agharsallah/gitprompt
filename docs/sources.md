# Prompt sources — research notes

What each tool documents for capture, and what GitPrompt can get from it. Two kinds of evidence:
- **D (docs):** vendor documentation, re-checked **2026-11-07** (E6 step 1). Every claim links to its page; "unverified" means the docs don't say.
- **L (local):** structure inspected on the author's macOS machine on 2026-11-01 (keys/schemas only, no prompt content). Only relevant to the optional backfill add-on (ADR-0008), see the [appendix](#appendix--local-storage-backfill-add-on-only).

When a tool update changes an interface, update this file and add a fixture for the new version.

---

## Matrix (documented interfaces, 2026-11-07)

| Tool (version checked) | Interface | Config file (user level) | Prompt event | Session / prompt id | cwd | Model | Tokens / cost | Turn end | Subagents | Org policy that turns it off | Main gaps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **Claude Code** 2.1.317 | Hooks (GA) + OTel | `~/.claude/settings.json` (`hooks`, `env`) | `UserPromptSubmit` | `session_id` / `prompt_id` | yes | OTel | OTel (tokens + cost) | `Stop` | `agent_id`, `agent_type` | `disableAllHooks`, `allowManagedHooksOnly`, `strictPluginOnlyCustomization` | none (reference connector) |
| **Copilot CLI** 1.0.107 | Hooks (GA): command, HTTP; OTel (traces/metrics) | `~/.copilot/hooks/*.json` or `hooks` in `~/.copilot/settings.json` | `userPromptSubmitted` / `UserPromptSubmit` | `sessionId` / **none** | yes | **no** (OTel only) | **no** (OTel tokens, no cost) | `agentStop` / `Stop` | `agentId`, `agentType`, `parentAgentId` | policy files `/etc/github-copilot/policy.d/*.json`; `disableAllHooks` (per file / repo) | no prompt id, no model in hooks; camelCase form has no event name |
| **Copilot in VS Code** (agent mode) | Hooks (**Preview**), same files as Copilot CLI; OTel | same as Copilot CLI | `userPromptSubmitted` / `UserPromptSubmit` | as Copilot CLI | as CLI | OTel only | OTel tokens | `agentStop` / `Stop` | as CLI | enterprise-managed settings (key unverified); `chat.useHooks` is local-harness only | Preview; can't tell VS Code from CLI in the payload (unverified) |
| **Gemini CLI** 0.67.0 | Hooks (command only) + OTel | `~/.gemini/settings.json` (`hooks`) | `BeforeAgent` | `session_id` / **none** in hooks (`prompt_id` in OTel) | yes | `BeforeModel.llm_request.model` | OTel only | `AfterAgent` | none documented in hooks | `hooksConfig.enabled: false`; system `settings.json` overrides all | **Sunset for individuals** on 2026-06-18 (replaced by Antigravity CLI) |
| **Antigravity** (App, IDE, CLI; version not stated) | Hooks (command only) | `~/.gemini/config/hooks.json` | **none**: `lastUserInput` on every event | `conversationId` / `invocationNum` | `workspacePaths[]` | `modelName` | no | `Stop` | none in payloads | per-hook `enabled: false`; no enterprise key documented | no prompt-submit event (dedupe on `lastUserInput`) |
| **Cursor IDE** (1.7.x example) | Hooks (command, `prompt`) | `~/.cursor/hooks.json` | `beforeSubmitPrompt` | `conversation_id` / `generation_id` | `workspace_roots[]` | `model` | no | `stop` (`status`) | `subagentStart/Stop` (`subagent_id`, `parent_conversation_id`) | enterprise/team hooks run first; no "disable user hooks" key documented | no tokens/cost; no OTel |
| **Cursor CLI** (`cursor-agent`) | Hooks only partly (docs name `workspaceOpen` only) | `~/.cursor/hooks.json` | `beforeSubmitPrompt` (**unreliable**) | as IDE | as IDE | as IDE | no | `stop` (reported missing) | unverified | as IDE | forum reports: prompt/stop hooks missing in CLI, absent in `-p` |
| **Codex CLI** 0.167.0 | Hooks (command, `mcp_tool`; on by default) + OTel + `notify` | `~/.codex/hooks.json` or `[hooks]` in `~/.codex/config.toml` | `UserPromptSubmit` | `session_id` / `turn_id` | yes | `model` | OTel (tokens, no cost) | `Stop` (`last_assistant_message`) | `SubagentStart/Stop` (`agent_id`, `agent_type`) | `requirements.toml`: `[features].hooks = false`, `allow_managed_hooks_only = true` | docs just moved host; hooks were feature-flagged until recently |

Sources per row: see the per-tool notes below; every cell is backed there with a link.

## Capabilities per tool

What a connector can feed into the dashboard. Panels should show "not available for X" when a capability is `no`, instead of zeros (E6 step 2). Proposed vocabulary for a `capabilities:` list in the spec:

| Capability | Meaning | Claude Code (hooks + OTel) | Copilot CLI | Copilot VS Code | Gemini CLI | Antigravity | Cursor IDE | Cursor CLI | Codex CLI |
|---|---|---|---|---|---|---|---|---|---|
| `prompt` | prompt text on a prompt event | yes | yes | yes | yes | yes (`lastUserInput`, no submit event) | yes | unreliable | yes |
| `turn_id` | id that ties a turn's events to its prompt | yes (`prompt_id`) | **no** | **no** | no (OTel `prompt_id` only) | partial (`invocationNum`) | yes (`generation_id`) | unverified | yes (`turn_id`) |
| `cwd` | working directory | yes | yes | yes | yes | yes (`workspacePaths`) | yes (`workspace_roots`) | yes | yes |
| `model` | model per prompt/turn | yes (OTel) | no (OTel only) | no (OTel only) | yes (`BeforeModel`) | yes | yes | unverified | yes |
| `usage` | tokens | yes (OTel) | no (OTel only) | no (OTel only) | no (OTel only) | no | no | no | no (OTel only) |
| `cost` | money | yes (OTel `cost_usd`) | no | no | no | no | no | no | no |
| `turn_end` | an event when the answer is done | yes (`Stop`) | yes (`agentStop`) | yes | yes (`AfterAgent`) | yes (`Stop`) | yes (`stop`) | reported missing | yes (`Stop`) |
| `response` | final answer text | yes | no (only `subagentStop.response`) | no | yes (`prompt_response`) | no | yes (`afterAgentResponse.text`) | unverified | yes |
| `tools` | tool calls and results | yes | yes | yes | yes | yes | yes | partial (shell only) | yes |
| `subagents` | subagent start/stop + markers on their events | yes | yes | yes | no (`SubagentStop` mapping only in 0.68 preview) | no | yes | unverified | yes |
| `origin_markers` | can tell typed from generated | yes (tags, `UserPromptExpansion`, entrypoint) | partial (subagent fields; system notifications never fire the prompt hook; `userPromptTransformed`) | partial (as CLI) | no (only shared rules) | no | partial (`attachments`, subagent ids) | no | partial (subagent fields, `permission_mode`) |
| `transcript_path` | path string to the tool's transcript | yes | `agentStop`/`subagent*`/`preCompact` only | unverified | yes | yes | yes (null if disabled) | unverified | yes |
| `session_lifecycle` | session start/end | yes | yes | yes | yes | start only | yes | unverified | yes |

## Recommendation: the second connector is Copilot CLI

**Gemini CLI is out.** Google stopped serving Gemini CLI for Google AI Pro/Ultra and free Gemini Code Assist users on 2026-06-18 and points them to Antigravity CLI; only Standard/Enterprise licences and paid API keys keep it ([Google blog](https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/), banner on [geminicli.com/docs/hooks](https://geminicli.com/docs/hooks/)). Antigravity, its successor, has no prompt-submit event.

**Copilot CLI** has the more stable documented hooks of the plan's two candidates:
- GA reference on docs.github.com, and changes so far were additive: the snake_case form was added next to camelCase (v1.0.21), and `config.json` was dropped as a hooks file in favour of `hooks/*.json` ([reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference), [releases](https://github.com/github/copilot-cli/releases)).
- One user-level file also reaches the VS Code Copilot harness, which reads `~/.copilot/hooks/` ([VS Code hooks](https://code.visualstudio.com/docs/copilot/customization/hooks)).
- Safest by design: "Command and HTTP config-file `userPromptSubmitted` hooks have their output dropped, including `modifiedPrompt`" — a config hook can't change or block a prompt ([reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference)).
- Timeouts and non-zero exits are fail-open for every observational event ([reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference)).

**What it costs:** Copilot hooks have **no prompt id and no model**. Events can only join their prompt by session and order. Handling that is a generic capability (`turn_id: no` → attach a session's events to the latest prompt before them), the "generic capability handling" the E6 acceptance allows, not a Copilot branch. Model and tokens wait for an optional Copilot OTel source.

**Third connector: Codex CLI.** Its hooks copy Claude Code almost field for field (`session_id`, `turn_id`, `model`, `transcript_path`, `async`, `Stop.last_assistant_message`) and would need no Python at all (`prompt_id: [turn_id]`). It is not second only because its interface is the least settled: the docs moved from developers.openai.com to learn.chatgpt.com (308 redirect, 2026-11-07), and hooks were behind an experimental feature flag as recently as v0.114–0.116 (third-party reports, unverified).

### Draft spec: `copilot-cli.yaml` (not a real spec file yet)

Uses the PascalCase event names on purpose: the docs say the payload format is "selected by the event name used in the hook configuration", and only the PascalCase form sends `hook_event_name` and snake_case fields, which the shared `_defaults.yaml` map already covers. The camelCase form has no event-name field at all.

```yaml
# GitHub Copilot CLI: documented hooks (https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference)
# on observational events only. Command hooks, like claude-code.yaml: an HTTP hook to 127.0.0.1 needs
# COPILOT_HOOK_ALLOW_LOCALHOST=1 in the user's environment, which GitPrompt must not set.
id: copilot-cli
name: GitHub Copilot CLI
detect: ["${COPILOT_HOME:-~/.copilot}"]
version_cmd: [copilot, --version]

x-hook: &hook
  - type: command
    bash: >-
      curl -s -m 2 -H 'Content-Type: application/json' -H 'Authorization: Bearer {{token}}'
      --data-binary @- '{{receiver}}/ingest/{{source}}' >/dev/null 2>&1 || true
    powershell: "..."      # unverified: a Windows equivalent is needed before Windows support
    timeoutSec: 5          # no `async` option is documented; the forwarder returns in ≤2 s

install:
  # A file of our own: Copilot loads every ~/.copilot/hooks/*.json, so nothing of the user's is edited.
  # Invalid JSON or a bad `version` rejects the whole file (reference), so the installer must write it whole.
  file: "${COPILOT_HOME:-~/.copilot}/hooks/gitprompt.json"
  merge:
    version: 1
    hooks:
      UserPromptSubmit: *hook
      PostToolUse: *hook
      PostToolUseFailure: *hook        # PascalCase alias unverified
      SubagentStart: *hook
      SubagentStop: *hook
      Stop: *hook                      # = agentStop
      PreCompact: *hook
      SessionStart: *hook
      SessionEnd: *hook
      ErrorOccurred: *hook             # PascalCase alias unverified

format: json
prompt_events: [UserPromptSubmit]

capabilities: [prompt, cwd, turn_end, tools, subagents, session_lifecycle, origin_markers]
# missing: turn_id, model, usage, cost, response (see docs/sources.md)

map:
  parent_agent_id: [parent_agent_id, parentAgentId]
  stop_reason: [stop_reason, stopReason]
  # session_id, cwd, prompt, event, agent_id, agent_type, tool_name, transcript_path: from _defaults.yaml

origin:
  # Subagent events carry agent_id/agent_type: the shared rules already classify them.
  # userPromptSubmitted "never [runs] for system notifications" (reference), so no task-notification rule.
  - when: {field: parent_agent_id}
    origin: subagent
injected: []  # unverified: whether `prompt` holds expanded @-mentions; userPromptTransformed has the expanded form

policy:
  files:
    - "/etc/github-copilot/policy.d/*.json"
    - "C:/ProgramData/GitHub/Copilot/policy.d/*.json"
    - "${COPILOT_HOME:-~/.copilot}/settings.json"
  block_if:
    disableAllHooks: [true]   # documented for repo settings.json and per hooks file; user settings.json unverified
  # Registry policy (HKLM\Software\Policies\GitHub\Copilot) and enterprise-managed settings can't be read from disk.

terms: >-
  Uses GitHub Copilot CLI's documented hooks in a hooks file of your own (~/.copilot/hooks/gitprompt.json).
  Copilot drops the output of prompt hooks from config files, so GitPrompt cannot change or block a
  prompt. It does not read credentials, proxy traffic, or send anything off this machine. The same file
  is also read by the Copilot agent in VS Code.

documented:
  "*": [session_id, timestamp, cwd, hook_event_name, agent_id, agent_type, parent_agent_id]
  UserPromptSubmit: [prompt]
  PostToolUse: [tool_name, tool_input, tool_result]
  PostToolUseFailure: [tool_name, tool_input, error]
  SubagentStart: [transcript_path, agent_name, agent_display_name, agent_description]
  SubagentStop: [transcript_path, agent_id, agent_type, agent_name, agent_display_name, response, stop_reason]
  Stop: [transcript_path, stop_reason, stop_hook_active]
  PreCompact: [transcript_path, trigger, custom_instructions]
  SessionStart: [source, initial_prompt]
  SessionEnd: [reason]
  ErrorOccurred: [error, error_context, recoverable]
  # snake_case names are inferred from the camelCase schema; confirm against a live payload.

not_captured:
  preToolUse: A decision point (fail-closed, can deny a tool call). postToolUse has the same data.
  permissionRequest: A decision point (answers a permission prompt).
  userPromptTransformed: Holds the runtime-expanded prompt (injected context); the typed text comes from userPromptSubmitted.
  notification: CLI only, UI notices.
  sessionStart prompt hooks (type prompt): Would inject text into the session.
```

### Conformance fixtures: `tests/sources/fixtures/copilot-cli/`

Synthetic payloads built from the documented schemas (the docs give TypeScript types, not literal samples), all in the PascalCase/snake_case form unless noted. Each file pairs a payload with the expected mapped fields and origin.

| File | Checks |
|---|---|
| `user_prompt_submit.json` | prompt, session_id, cwd, event mapped; origin `typed`; `prompt_id` empty (no dedup) |
| `user_prompt_submit_slash.json` | `/model …`-style text → origin `slash_command` (shared rule) |
| `user_prompt_submit_subagent.json` | prompt inside a subagent (`agent_id`, `agent_type`, `parent_agent_id`) → origin `subagent` |
| `post_tool_use.json` | tool_name, `tool_input`, `tool_result.resultType: success`; attaches to the session |
| `post_tool_use_failure.json` | `error` string |
| `subagent_start.json` / `subagent_stop.json` | `agent_name`, `agent_id`, `agent_type`, `response`, `stop_reason` |
| `stop.json` | `stop_reason: end_turn`, `stop_hook_active: false`, `transcript_path` → turn end |
| `pre_compact.json` | `trigger: auto` |
| `session_start.json` | `source: startup` and `initial_prompt` (unverified: whether a `-p` prompt also fires `UserPromptSubmit`) |
| `session_end.json` | `reason: user_exit` |
| `error_occurred.json` | `error.message`, `error_context: model_call`, `recoverable` |
| `camelcase_user_prompt_submitted.json` | negative case: the camelCase form (`sessionId`, epoch-ms `timestamp`, no event name) maps session and prompt via `_defaults` aliases but has no `event`, so it is stored as an unnamed event, not a prompt (drift signal) |

## Check: does one broken user override take down the Sources page? Yes, and more

`load_specs` (`src/gitprompt/sources/spec.py`) raises `SpecError` on the first file that is not YAML or fails validation. The API resolves specs per request through `get_specs` (`src/gitprompt/api/deps.py`), and `SpecError` is answered with HTTP 500 by the shared handler in `api/app.py`. Only start-up indexing suppresses it.

Reproduced on 2026-11-07 with the test client in a temp dir: one `~/.gitprompt/sources/broken.yaml` (either invalid YAML, or valid YAML missing `detect`/`install`) makes **every** spec-dependent route return 500 with the file's error as `detail`:

- `GET /api/sources`, `/api/prompts`, `/api/fields`, `/api/security`: 500, so the Sources, Prompts, Fields and Insights pages all fail, not only Sources.
- `POST /ingest/claude-code`: 500, so **capture stops for every connected tool** until the file is fixed. The Claude Code forwarder discards errors by design, so the user sees nothing in their sessions; prompts in that window are lost.
- `/healthz`: 200.

Fix belongs to E6 step 5: load each file on its own, keep the good specs, and return the broken ones with their error (file name, bundled vs user) for Settings → Connectors. A broken override of a bundled spec should fall back to the bundled one and say so.

---

## Per-tool notes

### Claude Code — CLI 2.1.316 (also used by the VS Code / Cursor extension)
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
- A live run produced these telemetry types: `user_prompt`, `assistant_response`, `api_request`, `tool_decision`, `tool_result`, `hook_execution_start/complete`, `hook_registered`, `plugin_loaded`, `mcp_server_connection`, `managed_settings_resolved` (these lifecycle events are now received but not stored, see `drop_events` and ADR-0011), and the metrics `session.count`, `lines_of_code.count`, `commit.count`, `cost.usage`, `token.usage`, `code_edit_tool.decision` and `active_time.total`. Telemetry also attaches `user.email`, `organization.id` and account IDs (stored locally only).


### GitHub Copilot CLI — 1.0.107 (2026-11-06)
Docs: [hooks reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference), [releases](https://github.com/github/copilot-cli/releases).
- **Files:** user `~/.copilot/hooks/*.json` (or `$COPILOT_HOME/hooks/`) or `hooks` in `~/.copilot/settings.json`; `config.json` is no longer read for hooks. Repo: `.github/hooks/*.json`, `.github/copilot/settings*.json`, and repo `.claude/settings*.json`. Also plugins, custom-agent frontmatter, and machine policy.
- **Shape:** `{version: 1, disableAllHooks?, hooks: {<event>: [ {type: "command", bash|powershell|command|exec+args, cwd, env, timeoutSec} | {type: "http", url, headers, allowedEnvVars, timeoutSec} ]}}`. Flat handler list (no `matcher`/`hooks` nesting like Claude Code). Default timeout 30 s. No `async` documented.
- **HTTP hooks:** `https://` only, except `http://localhost`, `127.*`, `[::1]` when `COPILOT_HOOK_ALLOW_LOCALHOST=1` is set. So GitPrompt keeps the curl command forwarder.
- **Payload form:** chosen by the event name in the config. camelCase (`userPromptSubmitted`) → `sessionId`, epoch-ms `timestamp`, `cwd`, **no event-name field**. PascalCase (`UserPromptSubmit`, since v1.0.21) → VS Code form: `hook_event_name`, `session_id`, snake_case fields.
- **Events (14):** `sessionStart` (`source`, `initialPrompt?`), `sessionEnd` (`reason`), `userPromptSubmitted` (`prompt`), `userPromptTransformed` (`prompt`, `transformedPrompt`), `preToolUse`, `postToolUse` (`toolName`, `toolArgs`, `toolResult`), `postToolUseFailure` (`error`), `permissionRequest`, `agentStop` (`transcriptPath`, `stopReason`, `stop_hook_active`), `subagentStart`, `subagentStop` (`agentId`, `agentType`, `agentName`, `response`, `stopReason`), `errorOccurred` (`error`, `errorContext`, `recoverable`), `preCompact` (`trigger`, `customInstructions`), `notification`.
- **Subagent marker:** hooks inside a subagent gain `agentId`, `agentType`, `parentAgentId` (`agent_id`, … in the PascalCase form). Since v1.0.81 inputs also carry OTel `traceparent` ([release](https://github.com/github/copilot-cli/releases/tag/v1.0.81)).
- **Origin:** `userPromptSubmitted` "never [runs] for system notifications"; `userPromptTransformed` holds what the model receives. Whether `prompt` is exactly the typed text (unexpanded `@` mentions) is unverified.
- **Output:** config-file `userPromptSubmitted` output is dropped. Non-zero exit and timeouts fail open, except `preToolUse` (fail-closed).
- **Policy:** `/etc/github-copilot/policy.d/*.json`, `C:\ProgramData\GitHub\Copilot\policy.d\*.json`, registry `HKLM\Software\Policies\GitHub\Copilot`. `disableAllHooks` skips one file, or (repo `settings.json`) every non-policy hook. No `allowManagedHooksOnly` equivalent is documented.
- **OTel:** traces and metrics over OTLP/HTTP, enabled by `COPILOT_OTEL_ENABLED=true` or `OTEL_EXPORTER_OTLP_ENDPOINT`; `COPILOT_OTEL_ENDPOINT` wins; default protocol `http/protobuf` (receiver only decodes JSON); prompt content off by default (`COPILOT_OTEL_CAPTURE_CONTENT`); `gen_ai.usage.*` tokens, no cost, no prompt event ([VS Code monitoring guide](https://code.visualstudio.com/docs/copilot/guides/monitoring-agents)). A docs.github.com page for CLI OTel was not found; `copilot help monitoring` describes it.
- **Gaps:** no prompt id, model or tokens in hooks; turn grouping by order only. Repo `.claude/settings*.json` is read too, but GitPrompt only installs into the user-level `~/.claude/settings.json`, so no double capture.

### GitHub Copilot in VS Code (agent mode) — page dated 2026-11-04
Docs: [VS Code hooks](https://code.visualstudio.com/docs/copilot/customization/hooks).
- **Preview.** The "Copilot harness" loads the same files as Copilot CLI (`~/.copilot/hooks/`, `.github/hooks/*.json`, inline `hooks` in `~/.copilot/settings.json` and `.github/copilot/settings*.json`, repo `.claude/settings*.json`). Events: `sessionStart`, `userPromptSubmitted`, `preToolUse`, `postToolUse`, `preCompact`, `subagentStart`, `subagentStop`, `agentStop`, `sessionEnd`; PascalCase aliases give snake_case fields.
- `chat.useHooks`, `chat.hookFilesLocations`, `chat.useClaudeHooks` control only the *Local* harness, not the Copilot harness. Enterprise-managed settings can restrict hooks (key not named); "Claude and Codex harnesses are disabled when Copilot enterprise-managed settings apply".
- **Consequence:** the Copilot CLI connector's file also captures VS Code agent prompts. Telling them apart from the payload is unverified (candidates: `transcript_path`, the OTel `service.name`).
- OTel: see Copilot CLI; settings `github.copilot.chat.otel.*`, and enterprise-managed telemetry overrides user values ([monitoring guide](https://code.visualstudio.com/docs/copilot/guides/monitoring-agents)) → a GitPrompt OTel source must `block_if_set` on it.

### Gemini CLI — 0.67.0 (2026-11-04) — sunset for individual users
Docs: [hooks reference](https://geminicli.com/docs/hooks/reference/), [hooks overview](https://geminicli.com/docs/hooks/), [configuration](https://geminicli.com/docs/reference/configuration/), [telemetry](https://geminicli.com/docs/cli/telemetry/), [releases](https://github.com/google-gemini/gemini-cli/releases), [transition](https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/).
- **Status:** since 2026-06-18 Gemini CLI no longer serves Google AI Pro/Ultra and free Code Assist users; Standard/Enterprise licences and paid API keys keep it. Releases continue.
- **Files:** project `.gemini/settings.json` > user `~/.gemini/settings.json` > system `/etc/gemini-cli/settings.json` (macOS `/Library/Application Support/GeminiCli/settings.json`) > extensions. (The hooks page lists system lower; the configuration page says system settings override all files.)
- **Shape:** `hooks.<Event>[] = {matcher?, sequential?, hooks: [{type: "command", command, name?, description?, timeout (ms, default 60000)}]}`. Command only. stdout must be only JSON; exit 2 blocks.
- **Events (11):** `BeforeAgent` (`prompt`), `AfterAgent` (`prompt`, `prompt_response`, `stop_hook_active`), `BeforeModel` / `AfterModel` (`llm_request.model`, `llm_response`), `BeforeToolSelection`, `BeforeTool`, `AfterTool`, `SessionStart`, `SessionEnd`, `Notification`, `PreCompress`. Common: `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `timestamp`.
- **Enable keys:** `hooksConfig.enabled` (default `true`), `hooksConfig.disabled` (names). The old `tools.enableHooks` is no longer documented. Project hooks are fingerprinted and re-confirmed when they change.
- **OTel:** `telemetry.*` (default gRPC to `localhost:4317`, `logPrompts` defaults to **true**); events carry `session.id` and `prompt_id`, `api_response` has token counts.
- **Gaps:** no prompt id in hooks; `BeforeModel`/`AfterModel` would be needed for the model and carry the whole request (too much). No subagent markers (a `SubagentStop` mapping is in 0.68 preview).

### Antigravity — App, IDE and CLI (version not stated in docs)
Docs: [hooks](https://www.antigravity.google/docs/hooks), [Gemini CLI migration](https://antigravity.google/docs/gcli-migration).
- **Files:** `~/.gemini/config/hooks.json` (global) or `.agents/hooks.json` (workspace); the CLI also loads plugin `hooks.json`. `~/.gemini/antigravity-cli/settings.json` is not loaded for hooks.
- **Shape:** top-level object of named hook definitions, each `{enabled?, <Event>: …}`; tool events take `[{matcher, hooks: [...]}]`, others a handler list. `type: "command"` only, `timeout` seconds (default 30).
- **Events (6):** `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation` (`invocationNum`, `initialNumSteps`), `Stop` (`executionNum`, `terminationReason`, `fullyIdle`). Common: `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName`, **`lastUserInput`** (new since the last check: prompt text is now available, but on every event, with no submit event).
- **Gaps:** prompts must be derived by deduplicating `lastUserInput` per conversation (a spec feature we don't have); no tokens, no subagent markers, no enterprise policy documented; several events can inject steps, so only `PostToolUse`/`Stop`/`PostInvocation` are observational. Exit codes unverified.

### Cursor IDE
Docs: [hooks](https://cursor.com/docs/agent/hooks).
- **Files (priority):** enterprise `/Library/Application Support/Cursor/hooks.json` (macOS), `/etc/cursor/hooks.json`, `C:\ProgramData\Cursor\hooks.json` → team (dashboard, synced) → project `.cursor/hooks.json` (trusted workspaces) → user `~/.cursor/hooks.json`. All matching hooks run.
- **Shape:** `{version: 1, hooks: {<event>: [{command, type?: command|prompt, timeout?, matcher?, failClosed?, loop_limit?}]}}`. No HTTP type.
- **Events:** `sessionStart/End`, `beforeSubmitPrompt` (`prompt`, `attachments[{type: file|rule, file_path}]`), `preToolUse`, `postToolUse`, `postToolUseFailure`, `subagentStart` (`subagent_id`, `subagent_type`, `task`, `parent_conversation_id`, `subagent_model`), `subagentStop` (`status`, `summary`, `duration_ms`, `message_count`, `tool_call_count`, `modified_files`), `before/afterShellExecution`, `before/afterMCPExecution`, `beforeReadFile`, `afterFileEdit`, `preCompact`, `stop` (`status`, `loop_count`), `afterAgentResponse` (`text`), `afterAgentThought`, Tab events, `workspaceOpen`.
- **Common:** `conversation_id`, `generation_id`, `model`, `model_id?`, `model_params?`, `hook_event_name`, `cursor_version`, `workspace_roots`, `user_email`, `transcript_path` — all already in `_defaults.yaml` aliases.
- **Policy:** no documented key to disable user hooks; enterprise/team hooks only add. Can load Claude Code hooks (third-party hooks page not checked: unverified whether `~/.claude/settings.json` hooks fire in Cursor → risk of double capture with the Claude Code connector).
- **Gaps:** no tokens, no cost, no OTel. `stop.followup_message` is auto-submitted, so only ever answer `{}`.

### Cursor CLI — `cursor-agent`
- Docs only say `workspaceOpen` "Runs in the Cursor desktop app and CLI" ([hooks](https://cursor.com/docs/agent/hooks)).
- Forum reports (not vendor docs): `beforeSubmitPrompt`, `afterAgentResponse` and `stop` not invoked in CLI 2026.08.11 ([thread](https://forum.cursor.com/t/cursor-cli-omits-beforesubmitprompt-afteragentresponse-and-stop-hooks-loses-token-usage-and-emits-inconsistent-generation-id-values/169059)); in interactive mode `beforeSubmitPrompt` fires only if a user/project file defines it, never with `-p` ([thread](https://forum.cursor.com/t/cursor-cli-does-not-run-enterprise-or-team-beforesubmitprompt-hooks/173219)). **Not a connector candidate until documented.**

### OpenAI Codex CLI — 0.167.0 (2026-11-06)
Docs: [hooks](https://learn.chatgpt.com/docs/hooks) (redirected from developers.openai.com/codex/hooks), [advanced config / OTel](https://learn.chatgpt.com/docs/config-file/config-advanced), [releases](https://github.com/openai/codex/releases).
- **Status:** hooks on by default; `[features].hooks = false` turns them off (`codex_hooks` is a deprecated alias). Third-party guides (unverified) say hooks arrived experimental in v0.114 behind a flag ([guide](https://codex.danielvaughan.com/2026/03/26/codex-cli-hooks-deep-dive)).
- **Files:** `~/.codex/hooks.json` or inline `[hooks]` in `~/.codex/config.toml`; project `.codex/` (trusted only); plugins; managed via `requirements.toml`. All layers run.
- **Shape:** Claude-like: `hooks.<Event>[] = {matcher?, hooks: [{type: command|mcp_tool, command, timeout, async, statusMessage, …}]}`. `async: true` supported (≤8 concurrent). No HTTP type.
- **Events:** `SessionStart/End`, `UserPromptSubmit` (`prompt`), `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreModelRequest`, `PostModelRequest`, `PreCompact`, `PostCompact`, `SubagentStart/Stop` (`agent_id`, `agent_type`, `agent_transcript_path`, `last_assistant_message`), `Stop` (`last_assistant_message`), `Interrupt`. Common: `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `model`, `turn_id`, `permission_mode`.
- **Policy:** `requirements.toml` `[features].hooks = false`, `allow_managed_hooks_only = true`. Exit 2 blocks on `UserPromptSubmit` and `Stop` — answer nothing.
- **OTel:** `[otel]` in user `config.toml` only; `exporter = otlp-http` (example protocol `binary`, JSON unverified); `log_user_prompt` off by default; events `codex.user_prompt`, `codex.api_request`, `codex.sse_event` (tokens on `response.completed`), `codex.tool_decision`, `codex.tool_result`; metadata has conversation id and model. Also `notify` (only `agent-turn-complete`).
- **Gaps:** config in TOML for OTel (installer only merges JSON; hooks can use `hooks.json`); no cost.

## Web: Copilot (github.com/copilot), Gemini (gemini.google.com)
- No local storage we can rely on → browser extension capturing typed text only (PLAN §4), iteration 3.
- Gemini history export: Google Takeout → *My Activity › Gemini Apps*. Copilot web export: none known.
- Later candidates via official exports: ChatGPT (Settings → Export), claude.ai (Settings → Export data).

## Cross-cutting observations
- **Hook convergence continues:** Codex copies Claude Code's field names and shape; Copilot added a Claude-compatible PascalCase/snake_case form and reads repo `.claude/settings*.json`; Cursor loads Claude Code hooks. `_defaults.yaml` aliases already cover most common fields.
- **Prompt ids are the main divergence:** Claude Code (`prompt_id`), Codex (`turn_id`) and Cursor (`generation_id`) have one; Copilot and Gemini don't. Turn grouping needs a generic fallback.
- **HTTP hooks are rare and restricted** (Copilot: https unless an env opt-in). Command forwarders stay the default.
- **Interfaces move fast:** in two weeks, Gemini CLI was sunset for individuals, Antigravity added `lastUserInput`, Codex hooks became default-on and its docs changed host. Re-check before each connector ships.

## Appendix — local storage (backfill add-on only)

Recorded 2026-11-01 (L). Not used by the hooks connectors; reading these is the optional backfill add-on of ADR-0008 and is subject to PLAN §4.

- **Copilot CLI:** `~/.copilot/session-state/<sessionId>/events.jsonl` (`session.start.data.context.{cwd, gitRoot, branch}`, `user.message.data.{content, transformedContent, interactionId, parentAgentTaskId}`, `assistant.message.data.model`), `workspace.yaml`, `~/.copilot/session-store.db` (SQLite `sessions`, `turns`).
- **VS Code Copilot Chat:** `workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl` mutation log (`kind` 0 snapshot / 1 set / 2 splice) plus legacy `.json` snapshots; request has `requestId`, `message.text`, `modelId`, `modeInfo`, `agent`.
- **Gemini CLI:** `~/.gemini/tmp/<project>/logs.json` (user prompts only), `chats/`, `~/.gemini/projects.json`.
- **Antigravity:** `brain/<conversationId>/.system_generated/logs/transcript.jsonl` (`USER_INPUT`, `PLANNER_RESPONSE`, …); IDE `conversations/<id>.db` (protobuf); App `conversations/<id>.pb` is encrypted (not used).
- **Cursor IDE:** `globalStorage/state.vscdb` `cursorDiskKV` (`composerData:*`, `bubbleId:*` with `type` 1 user / 2 AI, `tokenCount`), `composerHeaders` (`isSubagent`).
- **Cursor CLI:** `~/.cursor/chats/<workspaceHash>/<agentId>/store.db` content-addressed blobs, some encrypted.
- **Codex CLI:** `~/.codex/sessions/**/*.jsonl` (`session_meta`, `turn_context`, `event_msg`, `response_item`).

## References
- Claude Code: [hooks](https://code.claude.com/docs/en/hooks), [settings](https://code.claude.com/docs/en/settings), [managed settings](https://code.claude.com/docs/en/managed-settings), [monitoring](https://code.claude.com/docs/en/monitoring-usage)
- Copilot CLI: [hooks reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference), [hooks tutorial](https://docs.github.com/en/copilot/tutorials/copilot-cli-hooks), [releases](https://github.com/github/copilot-cli/releases)
- VS Code: [agent hooks](https://code.visualstudio.com/docs/copilot/customization/hooks), [monitoring agents with OTel](https://code.visualstudio.com/docs/copilot/guides/monitoring-agents)
- Gemini CLI: [hooks reference](https://geminicli.com/docs/hooks/reference/), [hooks](https://geminicli.com/docs/hooks/), [configuration](https://geminicli.com/docs/reference/configuration/), [telemetry](https://geminicli.com/docs/cli/telemetry/), [transition to Antigravity CLI](https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)
- Antigravity: [hooks](https://www.antigravity.google/docs/hooks)
- Cursor: [hooks](https://cursor.com/docs/agent/hooks)
- Codex: [hooks](https://learn.chatgpt.com/docs/hooks), [advanced config](https://learn.chatgpt.com/docs/config-file/config-advanced), [releases](https://github.com/openai/codex/releases)
