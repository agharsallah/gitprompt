# 0014 — Insights dashboard: more of the index, split groupings, per-session telemetry

**Status:** accepted (implemented)
**Amends:** [0012](0012-prompt-index-and-stats.md) (new index columns and tables, `outcome` grouping, new filters, `split_by`/`order`, `/api/stats/telemetry`)

## Context
Plan epic E3 turns the Overview into a dashboard where every panel answers one question over every prompt the filters keep, and every number links to the prompts behind it. ADR-0012 gave `/api/stats` counts and sums over the index, but the panels also need: who a model call worked for, typed-text references, reuse, agentic and failure counters, session length, a second dimension (per project per day), and telemetry that has no prompt id (lines of code, commits, slash commands that never reach the hooks). The Prompts page must accept the same filters, or a chart can't be a doorway.

## Decision
1. **More values in the index, from the derivations** (still one place per value). New `TURN` values: `work_usage` (usage per telemetry `query_source`), `tool_kind_of`, `subagent_tool_calls`, `git_commits` (Bash `git commit`); `slash_command` also reads telemetry `user_prompt.command_name`. New `prompt_index` columns: repo name, branch, session title, typed-text hash (case and spacing ignored), code blocks / URLs / mentions of the *typed* text, secrets, API time and errors, tool time, subagents and their tool calls, files written, git commits, compactions, denials, rejections. New table `prompt_index_work` (usage per query source, with its kind: `main` = `repl_main_thread`/`sdk`/none, `subagent` = `agent:*`, `system` = anything else, `derivations.work_kind`).
2. **Values that depend on later prompts or the clock are SQL at query time**, not columns: `followed` (a later prompt in the session, via a `(session, id)` index) and `outcome` = done / failed / superseded / running (no end, sent in the last hour, as `outcome.ts`) / open. The `outcome` grouping now uses these words instead of the raw `ended_by`; nothing consumed the old keys.
3. **Telemetry that arrives per session** goes into two index tables, `session_metric` and `session_command`, read incrementally by event id on each `ensure()` (rebuilt with the index). `GET /api/stats/telemetry` sums them over the sessions of the prompts the filters keep, within `since`/`until`. Reading the raw events instead took 90–500 ms on the real DB (JSON of 16k metric rows); the tables answer in ~5 ms.
4. **`/api/stats` additions:** groupings `day_hour`, `session_length` (sessions by span from first prompt to the end of the last turn, over all their prompts), `size` (typed characters), `text` (reused texts), `branch`, `work`, `query_source`; banded keys are zero-padded lower bounds so they sort. `split_by` gives each group `parts` (the same measures per value of a second grouping; at most one of the two may have several values per prompt). `order` sorts by key or a measure. Each bucket has a `detail` (repo name, session title or first text, tool kind, a text excerpt) and new measures (see `api/routes/stats.py Bucket`).
5. **Filters** (`store.FILTERS`, shared with `/api/prompts`): `weekday`, `hour`, `command`, `tool`, `tool_kind`, `query_source`, `outcome` (list), `size`, `has` (list: code, url, mention, secret, commit, subagent, tool_failure, api_error, denial, compaction, no_follow_up), and `session` takes several ids. A test checks that each filter keeps exactly the prompts the stats count.
6. **The dashboard aggregates nothing.** The Overview, Projects and Sessions read `/api/stats` (Sessions then loads the turns of the sessions it shows by id); `recent.ts` and its 1,000-prompt cap are gone. TS keeps presentation only (empty days, week order, bands, labels).

## Consequences
+ Totals on the Overview equal `/api/stats` and the Prompts list for the same filters; no cap. Real DB copy: Overview renders in 0.65–0.8 s (21 parallel requests, each ≈ 17 ms server time; `/api/sources` is the slowest at 150 ms); Projects 0.25 s; Sessions ≈ 0.9 s (deriving 20 sessions' prompts).
+ E4 can reuse `text` (lineage seeds), `session` + `split_by` (project/session stories) and the doorway filters.
− Session telemetry is attributed per session, not per prompt: lines of code "in the sessions these prompts belong to".
− Slash commands run without a hook prompt are only known with telemetry connected.
− More columns make the index larger and every rebuild a little slower (real DB: 3.2 → 4.1 s).

## Alternatives considered
- **Attribute metric data points to prompts by time window:** needs a reindex of the previous prompt whenever a new one arrives, and OTel exports are batched, so the attribution would be guesswork.
- **Promote telemetry-only slash commands to prompt rows:** they have no text and would blur what a prompt is (ADR-0010).
- **A second `group_by` list (`group_by=a,b`) instead of `split_by`:** a flat list of pairs is harder to draw and to limit per group.
