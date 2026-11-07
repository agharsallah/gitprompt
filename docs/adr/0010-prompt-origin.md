# 0010 — Prompt origin and typed text, from spec rules, derived on read

**Status:** accepted (implemented: classification, API, display; filtering follows in F1)
**Amends:** [0008](0008-universal-capture-declarative-sources.md) (adds `origin` and `injected` to the spec schema), [0009](0009-metadata-events-telemetry-derived-on-read.md) (one more value derived on read)

## Context
A "prompt" is anything a source reports on its prompt event. In the local data about half of those rows were not typed by the user: Claude Code submits `<task-notification>` texts when a background task ends and `<agent-message …>` texts between agents, and headless runs (`claude -p`, the Agent SDK) also fire `UserPromptSubmit`. Typed prompts can carry context the tool added around them (`<ide_selection>`, `<system-reminder>`). Every count, chart and search was skewed by this, and the dashboard stripped some tags client-side for previews only.

## Decision
1. **Origin taxonomy.** Every prompt gets one `origin`:

   | origin | meaning | user activity? |
   |---|---|---|
   | `typed` | default: no rule matched | yes |
   | `slash_command` | text starts with `/command`, a `UserPromptExpansion` event, or an OTel `user_prompt` with `command_name` | yes |
   | `task_notification` | text starts with `<task-notification>` | no |
   | `agent_message` | text starts with `<agent-message` | no |
   | `subagent` | the prompt has `agent_id` / `agent_type` | no |
   | `programmatic` | the turn's OTel `user_prompt` has `app.entrypoint = sdk-cli` | separate (see 4) |
   | `unknown` | no text, nothing left after removing context, or no spec for the source | — |

2. **Rules are data in the spec.** Two new spec keys, validated by Pydantic in `sources/spec.py`:
   - `origin`: an ordered list of `{when: {...}, origin: ...}`. `when` conditions (all must hold): `event` (an event of the same turn with that name; then `path`/`equals` look at that event), `field` (a mapped field has a value), `path` (a raw payload path has a value), `equals` (…and equals this), `text_prefix`, `text_regex` (on the typed text). Unknown keys, empty conditions and bad regexes are rejected when the spec loads.
   - `injected`: tags of context blocks a tool adds before or after what the user typed.

   `_defaults.yaml` holds the shared rules (subagent fields, the `/command` pattern) and the `system-reminder` tag. A spec declares only its own markers; its rules are tried **before** the shared ones, first match wins, and its tags are added to the shared ones.
3. **Derived on read** by a pure function, `insights.origin.classify(prompt_row, turn_events, spec)` → `{origin, typed_text, injected: [{kind, chars}]}`. Only leading and trailing `<tag …>…</tag>` blocks are removed (a block in the middle is part of what was typed; an unclosed tag is left alone). Text rules run on the typed text, so a reminder in front of a task notification doesn't hide it. A changed rule reclassifies old prompts with no migration. It needs the spec, so it is not one of the `derivations` registries (those take only the text or the turn); its descriptions are listed on the Fields page through `derivations.DERIVED`. The prompt API returns `origin`, `typed_text` and `injected` next to the unchanged `prompt`.
4. **`programmatic`: checked against the local data before keeping it.**
   - The IDE extensions report their own entrypoint (`claude-vscode`), not `sdk-cli`. A VS Code *terminal* reports `cli`.
   - The entrypoint never changes within a session (no session had two values).
   - `sdk-cli` sessions look headless: one user prompt per session; most are builtin slash commands that never reach the hooks as prompts; the hook prompts in them come from tool scratch directories (an evaluation harness, a plugin), one per session.
   - Join: the OTel `user_prompt` event carries `prompt.id`, the same value as the hook's `prompt_id`, so it is already among the turn's events. No session-level guess is needed.
   - Limit: prompts from sessions without telemetry can't be recognised and stay `typed`. "Programmatic" says *a program sent it*, not whether a person wrote the text inside that program, so it is shown separately.
5. **Owner-question defaults** (applied once filtering exists, F1): task notifications and agent messages are **not user activity**: hidden by default, shown inside session stories. `programmatic` is shown separately and hidden by default. `subagent` and `unknown` are hidden by default too.
6. **Display.** System-originated prompts are muted and labelled with an icon plus a label (`web/src/lib/prompts/origin.ts`, `OriginBadge.svelte`), never colour alone. The typed text is shown as the prompt; injected context is a collapsible note showing the prompt as received.

## Consequences
+ Views can default to "what I typed"; previews no longer need client-side tag stripping.
+ A new tool declares its own markers in YAML; tests cover every origin and the context cases.
− Classification runs per row on read, so it can't be used in SQL `WHERE`/`GROUP BY` yet. F1's rebuildable index should store the output of `classify` and rebuild when rules change.
− `programmatic` depends on the telemetry source being connected.

## Alternatives considered
- **Store origin at ingest:** fast filters, but every rule change would need a backfill. The F1 index gets the speed without giving up rebuildability.
- **Hard-code the markers in Python:** against "data over code"; a second tool would add `if source == …` branches.
- **Attribute `programmatic` by session id when the prompt has no matching `user_prompt`:** works because the entrypoint is per session, but no prompt needed it, so it was left out.
- **Strip context blocks anywhere in the text:** a user may paste or quote such a block on purpose; only the edges are tool-added.
