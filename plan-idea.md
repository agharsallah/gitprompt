# GitPrompt — Iteration 1c: from capture to insight

> **Who this is for:** agents (and humans) implementing the next features.
> **How to use it:** read §0–§2 before writing code. Pick work in the order of §5. Each epic has a goal, the data it can use, what to explore, what's out of scope, and acceptance criteria. When an epic says **Explore**, investigate and choose, then record the decision (ADR for anything structural). Don't hand back options.
> The original brief is kept verbatim in the [appendix](#appendix--original-brief).

---

## 0. Ground rules (apply to every epic)

These come from [PLAN.md](PLAN.md) §3, §4 and §9 and are **non-negotiable**:

1. **Local only.** Nothing leaves the machine: no CDN, no telemetry, no hosted sharing. "Share" means *export a file the user sends themselves* (see E5).
2. **Documented interfaces only** for capture. A new connector never reads private files, tokens or traffic.
3. **Raw is truth, everything else is derived and rebuildable.** A new classification or metric must apply to old prompts too. If you need to store a derived value for speed, it goes in a **rebuildable index** (see F1), never in place of raw.
4. **Data over code.** Per-tool behaviour belongs in the YAML spec (`src/gitprompt/sources/bundled/*.yaml`), not in `if source == ...` branches.
5. **Use the extension points that already exist** (PLAN §5 "Extending"): `derivations.TURN/PROMPT_STATS/SESSION`, `store.FILTERS`, `pipeline.DECODERS`, `probe.FILE_RULES`, `redaction.SECRETS`. Add a new registry only when none fits.
6. **Stack:** uv + FastAPI + stdlib `sqlite3` behind `Store`; pnpm + SvelteKit + Svelte 5 runes per [svelte.dev/llms.txt](https://svelte.dev/llms.txt). Reuse `web/src/lib/ui/*` and `web/src/lib/ui/charts/*` before you add a component; no chart library unless an ADR justifies it.
7. **Tests:** synthetic payloads only, temp dirs only. Backend in `tests/` (mirrors `src/`). Frontend logic in pure `.ts` modules with Vitest tests next to them (Vitest is installed but **no web test exists yet**: `aggregate.ts`'s docstring mentions `aggregate.test.ts`, which is missing).
8. **Done means** `just test` is green, the feature works against the real local DB (`just serve`), and PLAN.md / README / the Fields page are updated when behaviour changes.

---

## 1. What exists today (verified 2026-11-07)

### Backend (`src/gitprompt/`, ~2.1k LOC)
| Area | State |
|---|---|
| Capture | Claude Code hooks (20 events, async `curl` command hooks) + Claude Code OTel (`format: otlp`). One receiver, `POST /ingest/{source}` |
| Storage | `prompt` (fields/context/raw JSON), `event`, `annotation`; `prompt_view` adds turn number, previous time and next HEAD. Filters: `q` (LIKE), `source`, `session`, `repo`, `tag`, `starred`, `before` |
| Derived on read | `PROMPT_STATS` (size, slash command, mentions, URLs, secrets), `TURN` (response, tools, files, subagents, usage/cost), `SESSION` (lifecycle, metric totals, environment), commits after a prompt |
| Config | `core/config.py`: env vars only (`GITPROMPT_HOME`, `_PORT`, `_LOG_LEVEL`). **No user preferences layer.** |
| Spec schema | `sources/spec.py`: `detect`, `version_cmd`, `install`, `format`, `prompt_events`, `accept_events` (empty = keep all), `map`, `policy`, `terms`, `documented`, `not_captured` |

### Dashboard (`web/src/`, ~9k LOC)
Routes: **Overview** (`/`), **Prompts**, **Sessions**, **Projects**, **Sources**, **Fields**, **Design system**. A full chart kit already exists (`BarList`, `ColumnChart`, `Heatmap`, `Lanes`, `Meter`, `SegmentedBar`, `Sparkline`, `Tooltip`, `Legend`) and the Overview already shows totals, activity by day/hour, a week heatmap, tool kinds, model mix, "craft", length vs work, attention and lanes.
**All aggregation happens in the browser** (`lib/insights/aggregate.ts`) over at most **1,000 prompts** (`lib/api/recent.ts` `CAP`), fetched 200 at a time.

### The real data (local DB, 5 days of use)
| Fact | Value | Why it matters |
|---|---|---|
| Rows in `prompt` | 542 across 109 sessions, 15+ repos | — |
| …of which `<task-notification>` | **266 (49%)** | Injected by the harness when a background task ends. **Not typed by the user.** |
| …of which `<agent-message from=…>` | ~20 | Messages between agents. **Not typed.** |
| …with IDE context prepended (`<ide_selection>`) | ≥1 | User text **plus** injected context in one string |
| Duplicate prompt texts | 542 rows, 473 distinct | Reuse signal (and retry signal) |
| Subagent prompts (Agent tool `tool_input.prompt` in `PostToolUse`) | 50 | Written by the model, stored as events, **not** in `prompt` |
| Tool events from subagents (`agent_id` set) | 2,695 / 6,882 `PostToolUse` | Work done by subagents is mixed with main-thread work |
| OTel `user_prompt` events | 1,615; 1,282 are slash commands (`command_name`, `command_source` = builtin/custom) | Slash commands the hooks never see as prompts |
| OTel `app.entrypoint` | `cli` 1,079 · `sdk-cli` 535 · `claude-vscode` 2 | `sdk-cli` likely means programmatic/headless runs: **verify** |
| `api_request.query_source` | `repl_main_thread` $314 · `agent:*` $136 · `compact`, `hook_prompt`, `agent_summary`, `generate_session_title`, `prompt_suggestion`… | Cost can be split between user-driven and system-driven work |
| Models | opus-5-5 4,724 req / $456 · haiku-5-5 1,189 / $4 · sonnet-5-5 313 / $17 | "Most used model" by requests ≠ by cost |
| DB size | **348 MB** | ~110 MB is OTel noise: `hook_registered` (53k rows), `plugin_loaded` (20k), `hook_execution_*` (30k), `mcp_server_connection` (8k) |

**Conclusion:** half of what the app calls a "prompt" today was not typed by the user, so every count, chart and search is skewed. **Fix that first (F0).**

---

## 2. Vocabulary (use these words in code, UI and docs)

| Term | Meaning |
|---|---|
| **Prompt** | A row in `prompt`: anything a source reported as a prompt event |
| **Origin** | Who produced a prompt's text (taxonomy in F0) |
| **Typed text** | The part of a prompt the user actually typed, with injected context removed |
| **Turn** | A prompt plus the events that share its `prompt_id` |
| **Session / Project** | Events sharing `session_id` / prompts sharing a `git_root` (or `cwd` when there's no repo) |
| **Story** | An ordered, explained view of typed prompts over time for one scope (project, session, or prompt lineage). See E4 |
| **Connector** | User-facing name for a source spec plus its connection state. Keep `source` in code |

---

## 3. Foundations (build before the epics)

### F0 — Separate what the user typed from what the system generated *(brief item 7, prerequisite for E3/E4/E5)*

**Goal:** every prompt carries an `origin`, and user-typed text is separated from injected context, so every view can default to "what I typed".

**Origin taxonomy (start here, extend if the data demands):**
| `origin` | Signal in today's data | User-initiated? |
|---|---|---|
| `typed` | default when no other rule matches | yes |
| `slash_command` | text starts with `/`; `UserPromptExpansion` event; OTel `user_prompt.command_name` | yes |
| `task_notification` | text starts with `<task-notification>` | no |
| `agent_message` | text starts with `<agent-message` | no |
| `subagent` | `agent_id`/`agent_type` present on the prompt payload | no |
| `programmatic` | OTel `app.entrypoint` = `sdk-cli` (**verify first**: check whether IDE extensions also report `sdk-cli`) | ambiguous, show separately |
| `unknown` | rule engine failed or data missing | — |

Also split **injected context** out of typed prompts: leading/trailing tagged blocks such as `<ide_selection>…</ide_selection>`, `<ide_opened_file>`, `<system-reminder>`. Output: `typed_text` plus `injected: [{kind, chars}]`.

**How (follow the principles):**
- Rules are **data in the spec**, e.g. a new `origin:` block (an ordered list of `{when: {path|text_prefix|text_regex: …}, origin: …}`), with shared rules in `_defaults.yaml`. Another tool's spec then only declares its own markers.
- Evaluation is a pure function in `insights/` that is unit tested on synthetic payloads covering each row of the table above.
- Expose `origin`, `typed_text` and `injected` on `PromptOut`, and add `origin` (multi-value) to `store.FILTERS` → requires F1.
- Subagent prompts (Agent tool input) and system model calls (`query_source`) are **not** promoted to `prompt` rows. Surface them as children of the turn, attributed as `system` work (cost and tokens split into user-driven vs system-driven).

**UI:** a global **origin filter** (default: `typed` + `slash_command`) remembered per browser, and a distinct, consistent visual treatment for system-originated items (muted style plus an icon and a label, never colour alone).

**Acceptance:** with the current DB, the default Prompts list shows ≈ 250 prompts, not 542; no `<task-notification>` appears unless its origin is selected; reclassifying after a rule change needs no migration; tests cover every origin and the context-stripping cases.

### F1 — A rebuildable index for filtering and aggregation

**Problem:** derived values are computed in Python per row, but filters (`origin`) and statistics need SQL (`WHERE`, `GROUP BY`) over *all* prompts, not the newest 1,000 in the browser.

**Recommended direction (confirm with an ADR, e.g. 0010):** a `prompt_index` table (prompt id → origin, typed_text, model(s), cost, tokens, response_s, outcome, tool counts, repo, day…) plus `rules_version`. It is filled at ingest and **rebuilt** (like `remap`) when derivation or origin rules change. Raw stays the source of truth; the index is a cache you can drop at any time. Add SQLite **FTS5** over `typed_text` for search (replacing `LIKE`).
**Then:** `GET /api/stats` with the same filter parameters as `/api/prompts` plus `group_by` (day/hour/model/repo/origin/tool kind/…), and move Overview aggregation server-side. Keep pure TS helpers only for presentation.
**Explore:** benchmark on the real DB (and on a synthetic 100k-prompt DB): target < 200 ms for a 90-day stats query.

### F2 — Storage hygiene

- Stop storing OTel lifecycle noise: set `accept_events` (or add a `drop_events` list) in `claude-code-otel.yaml` for `hook_registered`, `plugin_loaded`, `hook_execution_*`, `mcp_server_connection`, `retention_sweep`, `managed_settings_resolved`. **Check first** that no derivation or Fields entry relies on them.
- A one-off `gitprompt compact` (or a Settings action) that deletes already-stored noise and runs `VACUUM`.
- Retention setting (keep raw events N days; prompts and annotations forever by default).
- **Acceptance:** DB growth per day drops by more than half and nothing on any page changes.

---

## 4. Epics

Each epic: **Goal → Uses → Explore → Out of scope → Acceptance.**

### E1 — Settings page *(brief item 1, and "manage connectors" from item 6)*

**Goal:** one `/settings` page for everything the user can configure, grouped into sections: **General** (theme, default time range, default origin filter, week start/timezone), **Connectors** (E2 lives here or is linked as a sub-page), **Capture** (events to keep, retention, telemetry noise filter from F2), **Privacy** (auto-hide detected secrets on ingest, strip absolute paths/emails in exports), **Data** (DB size and location, compact/vacuum, export all, delete by source/date with confirmation, backups list), **Security** (rotate receiver token → reconnect flow).
**Uses:** a new preferences layer. Recommended: `~/.gitprompt/preferences.json` validated by a Pydantic model, precedence **env > file > defaults**, `GET/PUT /api/settings` behind `require_dashboard`. Browser-only preferences (theme, collapsed panels) may stay in `localStorage`, but list them in one place.
**Explore:** which existing hard-coded constants should become settings (`CAP`, `RUNNING_WINDOW_MS`, clip length, ranges). Promote only those a user would plausibly change.
**Out of scope:** multi-user accounts, remote config.
**Acceptance:** every setting shows its current value and where it comes from (env/file/default); env-locked settings are shown read-only with the reason; a change applies without a restart (or says clearly that it needs one); destructive actions use a dry-run/confirm pattern like Connect.

### E2 — Connectors UX *(brief item 2)*

**Goal:** at a glance, the user knows each connector's state and the one action they can take now.
**State model (derive from `SourceOut`, make it explicit and exhaustive):** `not_installed` → `available` → `connected · receiving` / `connected · quiet` (no event for X) / `connected · drift` (`unmapped` > 0) / `outdated` (spec changed) / `blocked_by_policy` / `warning`. Each state has one icon, one label, one tone, one primary action and an explanation. Never rely on colour alone.
**Ideas to evaluate:** a last-event heartbeat with a sparkline of events per hour; a "Send test event" button that runs the documented hook path and confirms receipt; a stepper for multi-part connectors (hooks + telemetry shown as one tool with two optional layers); a diff viewer with the added lines highlighted; a "what this connector collects" list generated from `documented` + `not_captured`; per-connector counts by origin.
**Explore:** watch a first-time user flow (Overview empty state → Sources → Connect → first prompt) and remove every step that isn't needed.
**Out of scope:** changing what gets installed (that's spec work).
**Acceptance:** every state above is reachable in a test fixture and screenshotted in the Design system page; connecting from zero to the first captured prompt needs ≤ 3 clicks plus one confirmation; keyboard and screen-reader accessible.

### E3 — Insights dashboard *(brief item 3)*

**Goal:** turn the Overview into an insight dashboard backed by F1, filterable by **origin**, source/tool, project, model and time range, with every chart answering one named question.
**Questions to answer (each one is a panel; drop any the data can't support):**
- *Activity:* how many prompts I type per day/hour and when (heatmap); how many sessions; how long sessions last; streaks.
- *Prompt types:* mix by origin; slash commands used (OTel `command_name`); prompt size distribution; prompts with code/URLs/mentions; reuse rate (duplicate typed texts).
- *Models & cost:* requests vs tokens vs cost per model (show all three; "most used" depends on the measure); cache hit share; cost split between user-driven work, subagents and system calls (`query_source`); cost per typed prompt and per project.
- *Performance:* response time p50/p90; API time vs tool time; errors and `StopFailure`; API errors and refusals; compactions.
- *Agentic work:* tool kinds; subagent types and their share of work/cost; MCP servers; permission denials and rejections.
- *Outcomes:* commits after prompts, lines of code (OTel metric), prompts with no follow-up, abandoned/superseded prompts (`insights/outcome.ts`).
**Rules:** follow the existing chart kit and design tokens; load the `dataviz` skill before designing new chart types; each panel has an empty state and says when it needs telemetry; numbers link through to the filtered Prompts list (every chart is a doorway, not a dead end).
**Out of scope:** predictive or "AI-generated insights" text.
**Acceptance:** default filter excludes system origins; totals match `/api/stats` exactly; no 1,000-prompt cap; page loads in < 1 s on the real DB; aggregation logic has unit tests (Python for SQL, Vitest for presentation helpers).

### E4 — Prompt stories *(brief item 4)*

**Goal:** show how **the user's own typed prompts** evolve and get used over time, at three zoom levels:
1. **Project story** (`/projects/[id]`): a timeline of typed prompts per repo, grouped into phases (days/branches), with commits made after each prompt, files touched, cost, and outcomes. Answers "how did this project get built, prompt by prompt?"
2. **Session story** (`/sessions/[id]`): a narrative of one session: typed prompts as chapters, each with its outcome, tools and diffstat; system items (task notifications, subagent prompts, agent messages) as **collapsed, visually distinct** markers between chapters.
3. **Prompt lineage**: near-duplicate and refined versions of a prompt (retries in a session, the same instruction reused across projects), with how the wording changed (inline diff) and whether the outcome improved.
**Search:** FTS5 over `typed_text` (F1), with result snippets, filters (project, date, origin, tags) and "show in story".
**Explore:**
- Similarity: start with normalized text + trigram/Jaccard or MinHash in Python. Local embeddings only if an ADR shows a clear quality gain, with no network and no heavy dependency by default.
- Phase detection: date gaps, branch changes, `CwdChanged`, compactions.
- Which "evolution" metrics are meaningful (length, specificity, mentions/code blocks, retries before success).
**Out of scope:** reading transcripts to reconstruct conversations (forbidden by PLAN §4).
**Acceptance:** stories never show system-originated text as if the user typed it; every story element links to the prompt detail; works for a project with 200+ prompts without jank (virtualize or paginate).

### E5 — Share and export *(brief item 5)*

**Goal:** turn any story, dashboard view or prompt selection into a file the user can send, safely.
**Formats:** Markdown report; a **self-contained HTML** file (inline CSS and SVG charts, no external requests); CSV/JSON of the underlying rows; PNG/SVG of a single chart.
**Safety by construction:** an export preview screen showing exactly what will be included, with toggles: include prompt text or only stats; hide secrets (always on); replace absolute paths with repo-relative ones; drop account email/org/user ids (they come from telemetry resource attributes); drop raw payloads. Defaults are the most private option.
**Out of scope (needs an owner decision + ADR):** hosted share links or any upload. That contradicts the "nothing leaves the machine" non-goal.
**Acceptance:** an exported HTML opens offline in a browser with no network requests (test it); a redaction test proves that secrets, emails and home paths don't appear in any format.

### E6 — Modular connectors *(brief item 6)*

**Goal:** adding a tool is one YAML file plus fixtures, and the UI adapts to what that tool can provide.
**Steps:**
1. **Research matrix** (update [docs/sources.md](docs/sources.md), which already has notes for Copilot CLI/VS Code, Gemini CLI, Antigravity, Cursor/Cursor CLI, Codex). For each tool and version: documented interface (hooks/OTel/none), doc URL, prompt event, events available, fields (session, prompt id, cwd, model, tokens, cost, turn end, subagent markers), origin markers, policy/managed-settings keys, and gaps. Re-check vendor docs; they move fast.
2. **Capabilities in the spec:** declare what a connector yields (`prompt`, `turn_end`, `tools`, `usage`, `subagents`, `origin_markers`…), so panels show "not available for Cursor" instead of zeros.
3. **Conformance kit:** a `tests/sources/fixtures/<tool>/` folder of synthetic payloads built from documented examples, and one parametrized test that checks each spec maps prompt/session/prompt_id/event and classifies origin correctly. A new connector is "done" when its fixtures pass.
4. **Command shim** for tools without HTTP hooks (PLAN §8 iteration 2), stdlib only.
5. **Management in Settings → Connectors** (E1/E2): enable/disable, view the spec in use (bundled vs user override), show validation errors per file.
**Check:** `load_specs` raises on the first invalid spec. Verify whether one broken user override currently takes down the whole Sources page; if it does, isolate failures per spec.
**Acceptance:** the second tool (recommend **Copilot CLI** or **Gemini CLI**, whichever has the more stable documented hooks) works end to end with no Python changes beyond generic capability handling; Fields and Insights label which tool each value comes from.

---

## 5. Order of work

```
F0 origin ──► F1 index + stats API ──► E3 insights ──► E4 stories ──► E5 export
   │                                      ▲
F2 hygiene (parallel)          E1 settings ──► E2 connectors UX ──► E6 next connector
```

| Milestone | Contents | Exit criterion |
|---|---|---|
| **M1 Truth** | F0, F2, origin filter in Prompts | Default views show only user-initiated prompts; DB growth halved |
| **M2 Scale** | F1, `/api/stats`, FTS search | Overview has no cap and loads in < 1 s |
| **M3 Control** | E1, E2 | Every setting and connector state visible and actionable |
| **M4 Insight** | E3 | Every question in E3 answered or explicitly dropped with a reason |
| **M5 Story** | E4, then E5 | A project story can be exported to an offline HTML file |
| **M6 Reach** | E6 | Second tool captured end to end |

Keep each PR to one milestone slice, reviewable in under 30 minutes.

---

## 6. How to work (for implementing agents)

- **Start by reading:** PLAN.md, ADR-0008/0009, the module you will touch and its tests. Query the real DB read-only (`sqlite3 -readonly ~/.gitprompt/gitprompt.db`) to check your assumptions against real payloads, but **never copy real prompt text into fixtures or docs**.
- **Prove the data before you build the UI:** for any new metric, write the SQL or the derivation first, run it on the real DB, and sanity-check the numbers.
- **Prefer extending a registry** over a new mechanism; if you add a mechanism, document it in PLAN §5 "Extending".
- **Decisions:** anything that changes storage, the API shape, the spec schema or a guardrail gets an ADR in `docs/adr/` (next number: 0010).
- **Be creative inside the constraints:** propose views or metrics the brief didn't ask for when the data supports them. Mention them in the PR, kept behind the same quality bar.
- **Report honestly:** list in the PR what you verified (tests, real-DB check, screenshots) and what you didn't.

## 7. Open questions for the owner

1. Is `sdk-cli` traffic (535 prompts) "mine"? Default it to shown or hidden?
2. Should task notifications count toward activity at all, or appear only inside session stories?
3. Hosted sharing: stay file-only (recommended), or revisit the local-only non-goal later?
4. Local embeddings for prompt similarity: acceptable dependency, or text similarity only?
5. Retention defaults: keep raw events forever, or e.g. 90 days?

---

## Appendix — original brief

- We want to have one settings page to change the application preferences and user configurations.
- Make the design and the functionality for the connectors more ergonomic and user-friendly, and add clear visual cues for the connection status and available actions.
- Create a statistics and overview dashboard to provide insights into application usage, performance metrics, usage, most used model, user activity and all what you can get from the fields we collected, and the prompts created, frequency of prompts and more insights of the prompts as well as each type and aggregation of prompts. Add visualizations and charts to make the data more comprehensible and actionable. Allow to filter by type of prompt.
- Create stories with the prompts: show the context, usage, how the prompts evolve — of what the user has typed, not the other types such as subagent prompts — and allow to search by prompt or see the prompts we wrote for a project to track their development and usage over time. Make them visual and provide visualizations of the evolution and usage patterns of the prompts.
- Allow to share the stories and visualizations with other users or export them for reporting and collaboration purposes.
- Explore how we can add other connectors modularly to extend the application's capabilities and integrate with additional services or platforms, see the fields we can collect for these providers, add the necessary support for them in the application, and allow to manage these connectors from the settings page.
- Make sure we separate between what the user types and what the system generates, ensuring clear distinction and proper tracking of user input versus system output.
