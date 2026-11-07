# 0020 — Model-call usage from spec data, capabilities by version, interrupted turns

**Status:** accepted (implemented)
**Builds on:** [0017](0017-connector-capabilities.md) (capabilities), [0019](0019-codex-connector.md) (Codex connector)

## Context
An inventory of every field Codex sends (a scratch database from live `codex exec` runs) against what it documents ([hooks](https://learn.chatgpt.com/docs/hooks), [generated input schemas](https://github.com/openai/codex/tree/main/codex-rs/hooks/schema/generated) at `rust-v0.150.1`, `rust-v0.164.0`, `rust-v0.167.0` and `main`, [advanced config](https://learn.chatgpt.com/docs/config-file/config-advanced)) found four fields worth using:

1. **`PostModelRequest`** (since 0.164.0: the `model-request` schema is missing at `rust-v0.163.0` and present at `rust-v0.164.0`; 0.167.0 is the latest release): one event per model call with `status` (completed, failed, cancelled), `error`, `model`, `turn_id`, `agent_id`/`agent_type` and `token_usage` (`input_tokens`, `cached_input_tokens`, `cache_write_input_tokens`, `output_tokens`, `reasoning_output_tokens`, `total_tokens`). Run live with a scratch 0.167.0 (`pnpm add @openai/codex@0.167.0` in a scratch folder; the system install, 0.150.1, untouched): every call arrived with an async hook, including the last one, and a turn's calls add up to the usage `codex exec --json` prints for it. OpenAI's `input_tokens` include the cached ones; Anthropic's (Claude Code telemetry) do not.
2. **`Interrupt`**: Codex fires it when you stop a turn (seen live: Ctrl-C on `codex exec`), instead of `Stop`. The dashboard showed such a turn as "superseded" or "no end recorded".
3. **`SessionStart.source`** (`startup`, `resume`, `clear`, `compact`, `fork`; `fork` is new in 0.167.0): how a session began and each time it was taken up again (`codex exec resume` keeps the `session_id`).
4. **`permission_mode`** on every turn event: already mapped and shown, by Claude Code too, under the same name.

The derivations read model calls from telemetry `api_request` events with Claude Code's field names, so Codex's usage needed a mapping. And a capability that only a newer version of a tool has (usage) can't be a plain `capabilities` entry: on 0.150.1 the pages would show zeros instead of saying why.

What was not used, and why, is in [sources.md](../sources.md#codex-fields-captured-shown-and-why-not): `tool_response` has no exit code or duration (Bash sends only the command's output, a failed command included); reasoning effort, sandbox mode and approval policy are in no hook payload (only `config.toml` defaults and OpenTelemetry); `PreModelRequest` repeats `PostModelRequest` without the outcome.

## Decision
1. **A `usage` block in the spec** (`sources/spec.py` `Usage`): the events that report one model call each, and the payload path of each value GitPrompt knows (`model`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `reasoning_tokens`, `cost_usd`, `duration_ms`, `query_source`, `error`), plus `input_includes_cached` to subtract cached tokens from input tokens so every tool counts them the same way. `_defaults.yaml` holds Claude Code's telemetry names (`api_request`); a spec's own block replaces it whole. `derivations._usage` reads only through it; a call with an `error` counts as an API error. It is part of the index rules version.
2. **`capability_since`** in the spec: capability → the first tool version that yields it. `/api/sources` leaves such a capability out of `capabilities` when the installed version (`version_cmd`) is older, and lists it in `needs_version`; an unknown version is given the benefit of the doubt. `availability()` in the dashboard reports those tools as `updatable`, and panels say "Needs OpenAI Codex CLI 0.164.0 or later." instead of "not available".
3. **Codex:** subscribes to `PostModelRequest` (async; older versions ignore the name, checked live with 0.150.1), maps it with `usage`, and claims `usage` from 0.164.0. No `cost` (no hook reports it).
4. **`Interrupt` ends a turn** (`derivations.TURN_END`), and a new outcome **`interrupted`** groups it in SQL (`store.OUTCOME`), filters, mixes, rail nodes, lanes and badges. It ranks with superseded in lineage.
5. **`session.starts`** (a `SESSION` derivation): the `SessionStart.source` values in order. The Session tab says how the session started and how often it was taken up again.
6. Pages show cost and API time only when a tool reports them (Codex reports tokens only), and reasoning tokens when it does.

## Consequences
- Codex ≥ 0.164 users get tokens per turn, per model and on the Overview with no new layer, no TOML writing and no dependency. Users on older versions are told which version adds them.
- A new tool with model-call events is a `usage` block, not code. Claude Code is unchanged (the default block reproduces its telemetry names).
- `permission_mode` moved to the shared field map (`_defaults.yaml`).
- Not done: model calls as spans in the turn timeline (they carry no duration; the timeline still draws only telemetry `api_request`), subagent model calls inside the parent turn (they carry the subagent's own `turn_id`).
