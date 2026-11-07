# 0019 — Codex connector: hooks only, files from patches, assistant and after-connect steps

**Status:** accepted (implemented)
**Builds on:** [0008](0008-universal-capture-declarative-sources.md) (declarative specs), [0015](0015-connector-layers-and-states.md) (layers and states), [0017](0017-connector-capabilities.md) (capabilities, join, conformance)

## Context
OpenAI Codex CLI is the third connector. Its documented hooks ([hooks](https://learn.chatgpt.com/docs/hooks), [input schemas](https://github.com/openai/codex/tree/main/codex-rs/hooks/schema/generated)) copy Claude Code's shape and fields (`session_id`, `turn_id`, `cwd`, `model`, `transcript_path`, `Stop.last_assistant_message`), so most of the connector is data. Running it live against codex-cli 0.150.1 (`codex exec`, a scratch `CODEX_HOME` and a scratch GitPrompt daemon) showed what the docs don't say:

1. **Hooks must be trusted.** Codex skips a user hook until the user trusts it (`/hooks` in the CLI); trust is a hash of the hook, which includes the command and so GitPrompt's token. Connecting, an update and a token rotation each need a new review in Codex. GitPrompt must not write that trust itself: it is the user's review of what runs on their machine.
2. **An async `Stop` is lost** in `codex exec`: Codex cancels unfinished async hooks when the session ends, right after `Stop`.
3. **`apply_patch` names its files inside the patch** (`tool_input.command`), not in a path input, so a turn had no files written.
4. **Hooks carry the model, not usage.** The index only counted models from telemetry `api_request` events.
5. **Codex settings are TOML** (`config.toml`, `requirements.toml`); policy checks read JSON only.
6. **Subagent events carry the subagent's own `turn_id`** and a spawned subagent fires no `UserPromptSubmit`, so they join no prompt. Its tools are named `collaborationspawn_agent` / `collaborationwait_agent` (the docs say `spawn_agent`).

**Telemetry.** Codex can export OpenTelemetry as OTLP/HTTP JSON (`[otel] exporter = { otlp-http = { protocol = "json" } }`, in the source of 0.150.1; the docs' example shows `binary`), which the receiver decodes. A telemetry layer was evaluated and **not built**:
- It must be written into the user's `config.toml` (TOML; the standard library reads but cannot write it). That needs a dependency such as `tomlkit` (to keep the user's comments and layout) plus a TOML mode in the installer (record, strip, diff), for one connector.
- Tokens arrive on `codex.sse_event` (`response.completed`) with only `conversation.id`, no turn id. Log records are batched and flushed asynchronously (at shutdown at the latest), so joining them to "the latest prompt of the session" would put a session's tokens on the wrong turns. Only `codex.turn_cost` carries `turn.id`.
- Every record carries `user.email` and `user.account_id` (`log_event!` in `codex-rs/otel`), which GitPrompt would then store.
- The derivations read Claude Code's usage field names (`api_request.input_tokens`...); Codex's (`input_token_count`...) would need a usage mapping in the spec.
- Codex ≥ 0.167 documents `PostModelRequest` hooks with `token_usage` and `turn_id`: usage per model call through the hooks layer, joined by turn, with no TOML and no dependency. That is the better path, once it can be verified against an installed version (0.150.1 doesn't have it; it ignores unknown event names, so subscribing early would be harmless but untested).

## Decision
1. **`codex.yaml`, hooks layer only.** The curl command forwarder of `claude-code.yaml`, merged into the user's own `$CODEX_HOME/hooks.json` (default `~/.codex/hooks.json`; `config.toml` untouched) on observational events known to 0.150.1. `prompt_id: [turn_id]` (no `join: session`). `Stop`, `Interrupt` and `SessionEnd` run synchronously (SessionEnd always does; the others so `codex exec` doesn't cancel them); timeouts are 3 s, the maximum Codex allows for `Interrupt` and `SessionEnd`. `injected: [stdin]` (`codex exec` appends piped stdin as a `<stdin>` block). Capabilities: `prompt, cwd, model, turn_end, answer, tools, subagents, origin_markers, transcript_path, session_lifecycle`; no `usage`, `cost` or `session_metrics`, so pages say "not available for OpenAI Codex CLI".
2. **Spec schema, three optional fields** (`sources/spec.py`), all generic:
   - `tool_files`: tool name (`prefix*` allowed) → a pattern over the tool's text inputs whose first group is a file the call touched. `derivations.files_of` uses a path input first, else the pattern; turns, the prompt index and session stories (`storage.timeline` now returns each call's input) share it. Part of the index rules version.
   - `assistant`: who answers in the tool ("Claude", "Copilot", "Codex"), reported by `/api/sources`. Replaces the dashboard's hard-coded `AGENTS` map in `PromptDetail`.
   - `after_connect`: what the user still does in the tool once connected. The Connectors page shows it after connecting and in the `waiting` and `quiet` states. For Codex: trust the hooks with `/hooks`.
3. **Model without telemetry:** when a prompt's turn has no telemetry model usage, the index counts the prompt for the model its hooks reported (`fields.model`), with no usage. `/api/stats?group_by=model` then works for Codex; the models panel offers a Prompts measure (the only one without usage). Telemetry still wins when present.
4. **TOML policy files:** `probe` parses `policy.files` ending in `.toml` with `tomllib` (read-only). Codex's spec blocks on `features.hooks = false`, `features.codex_hooks = false` (deprecated alias) and `allow_managed_hooks_only = true` in `/etc/codex/requirements.toml`, the legacy `managed_config.toml`, the system and user `config.toml`.
5. **Exports leave out cost and tokens** when nothing reported usage, as the pages do, instead of `$0.00`.
6. **Conformance:** a fixture whose `turn` is null now checks that the event joins no prompt (it may carry an id of its own, like Codex subagent events).

## Consequences
- No new dependency, no installer change: `hooks.json` has Claude Code's JSON shape, so the existing merge/strip/record installer handles it, and disconnecting restores the user's file byte for byte (checked live).
- A Codex user has one extra step after connecting (and after every update or token rotation): trusting the hooks in Codex. Until then the connector stays `waiting` (or goes `quiet`) and says so.
- Subagent work in Codex is stored with its session but not counted in the parent turn (no parent turn id in the payload).
- Tokens and cost for Codex wait for the `PostModelRequest` hook (a usage mapping in the spec, then a fixture), not for a telemetry layer.
