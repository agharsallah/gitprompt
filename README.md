# GitPrompt

Keep every prompt you write to an AI tool, with its metadata, **locally**.

Iteration 1 supports **Claude Code** (terminal, IDE extensions, desktop app). See [PLAN.md](PLAN.md) for the design and roadmap, and [ADR-0008](docs/adr/0008-universal-capture-declarative-sources.md) for why it works this way.

## Quick start

```bash
just setup
just serve          # opens on http://127.0.0.1:7878
```

1. On the Overview, click **Connect**. The **Connectors** page opens with the exact change to `~/.claude/settings.json` in front of you; confirm it (a backup goes to `~/.gitprompt/backups/`).
2. Optionally add Claude Code's **Telemetry** layer too, for model, tokens and cost (applies to sessions started afterwards).
3. Type a prompt in any Claude Code session. It appears live under **Prompts**, on its session's rail. Each prompt names the tool that sent it (Claude Code, Codex, Copilot CLI) and its model, with tokens and cost when the tool reports them (and why not when it doesn't); filter by tool, model, outcome (done, failed, interrupted...) and project. Select it to see the turn as a story: what you asked, a timeline of model and tool calls, the answer, then tabs for activity (tools, files, commands), usage (tokens, cache, models), the repository (state at send time, commits afterwards), the session and every event. Star, rate, tag and annotate prompts there. Prompts you didn't type (task notifications, agent messages, subagent and programmatic runs) are muted and labelled with their origin, and context the tool added (an IDE selection, a system reminder) folds into a note under what you typed.
4. **Overview** is an insights dashboard over every prompt you keep in view, filtered by time range, origin (default: what you typed and slash commands), tool, project and model, all in the URL. Each panel answers one question: how much and when you prompt (per day, weekday × hour, session length, streaks), who wrote the prompts, which slash commands you run, how long your prompts are and what they reference, which prompts you send again, which model you use most by requests, tokens or cost, where the money goes (answering you, subagents, system calls; per prompt and per project), how fast and reliable turns are, what the agent does (tool kinds, subagents, MCP servers, denials) and what came of it (outcomes, commits, lines of code). Panels that need telemetry say so. Every bar, cell and row opens the matching prompts. **Sessions** and **Projects** group prompts by conversation and by repository, with no cap on how many prompts they cover.
5. **Stories** tell how your own typed prompts were used, never showing system text as if you typed it. Open a session from **Sessions** to read it as chapters: each prompt you typed with its outcome, tools, files, commits and cost, and the task notifications, agent messages, subagent briefs and compactions folded into quoted markers between them. Open a project from **Projects** to see it in phases (a new phase after a pause of 3 hours or more, or a lasting move to another branch), each with its commits, cost and outcome mix, plus a small view of how the project evolved. A prompt's **Versions** tab lists its retries in the session and the same instruction reused elsewhere, with the wording change marked word by word and whether the outcome improved.
6. **Search** finds prompts by the words you typed (every word must appear, short words too), filtered by project, time range, origin and tag. Matches are highlighted in a snippet, and each result opens the prompt or its place in the session and project stories.
7. **Export** (the button on a session, a project, the Overview and Prompts) saves what you see as a file you send yourself: Markdown, one self-contained HTML file (inline charts, no script, opens offline, follows light/dark, prints well), CSV or JSON of the rows, or a single chart as SVG or PNG. The preview is the file itself. By default it holds figures only: no prompt text, paths relative to the project and `~` for your home folder, no account email or ids, no raw payloads; secrets are always hidden. Nothing is uploaded ([ADR-0018](docs/adr/0018-export.md)).
8. **Fields** lists every captured field next to what the tool documents, to decide what to build on. **Design system** shows the tokens and components the dashboard is built from.
9. **Disconnect** removes only what GitPrompt added. After an upgrade, a connected layer may offer **Update connection** (with the diff first).

## How it stays within the tools' terms

GitPrompt uses only Claude Code's documented [hooks](https://code.claude.com/docs/en/hooks) and, if you connect it, its [OpenTelemetry export](https://code.claude.com/docs/en/monitoring-usage), in your user settings and pointed at this machine. It subscribes only to events that report what happened, never to decision points, and answers each hook with an empty response, so nothing is blocked or changed. It never reads credentials, intercepts traffic, reads Claude Code's private files, or sends anything off your machine. If your organization turns hooks off, GitPrompt shows that and doesn't install anything. Full rules: [PLAN.md §4](PLAN.md#4-terms-of-service-and-policy-guardrails).

## Connectors

The **Connectors** page (`/sources`) shows each tool as one card, with its connectors as steps: for Claude Code, **Hooks** (prompts, tools, turn ends) and the optional **Telemetry** layer (model, tokens, cost); for **GitHub Copilot CLI**, its hooks (also read by the Copilot agent in VS Code), installed as a file of its own, `~/.copilot/hooks/gitprompt.json`; for **OpenAI Codex CLI**, its hooks in `$CODEX_HOME/hooks.json` (default `~/.codex/hooks.json`), which Codex runs only once you trust them: after connecting (and after an update or a token rotation), open Codex and type `/hooks`; for **Cursor**, its hooks in `~/.cursor/hooks.json` (restart Cursor after connecting; your Cursor account email is removed before anything is stored). Each layer is in exactly one state, shown with an icon and a label (never colour alone), one sentence and the one action it needs now:

| State | Meaning | Action |
|---|---|---|
| Turned off | Turned off in GitPrompt: what it sends is discarded | Turn on |
| Not installed | The tool isn't on this machine | Check again |
| Ready to connect | Installed, not connected | Review and connect |
| Check before connecting | A setting may stop GitPrompt's entries from running (listed) | Review and connect |
| Blocked by policy | A setting GitPrompt won't override turns it off | Check again |
| Update available | GitPrompt now captures more than what is installed | Update connection |
| Payload drift | Prompts arrived without text: the tool's payload changed | Re-map stored events |
| Waiting for the first event | Connected, nothing received yet | Check again |
| Quiet | Connected, no event for a day | View prompts |
| Receiving | Events are arriving | View prompts |

A connected layer also shows its heartbeat (last event and events per hour over the last day), its prompts by origin, and **What this collects**: the events it stores, those it receives but drops, and those it never subscribes to, with the reason, straight from its spec. Each card also says what the tool can't provide (Copilot hooks have no model, tokens or cost; Codex hooks have the model, and tokens from Codex 0.164 on, but no cost: on an older Codex the pages say which version adds tokens), and **Spec in use** shows the spec file the layer runs on (bundled or your override, with its path and contents). Pages that need a capability a tool lacks say "not available for GitHub Copilot CLI" instead of showing zeros. **Turn off** keeps a connector's entries in the tool's settings but discards what it sends. Settings › Connectors summarizes the same states, with the spec file and Turn on/off for each layer. `/sources?connect=<id>` opens a connector's review directly (the Overview's first-run steps link there).

### Desktop apps, and where each prompt was typed

Every prompt shows its tool and, when the tool says, the app it was typed in, after a hairline on the same badge: **Claude Code | Desktop app**, **Claude Code | VS Code**, **Claude Code | CLI**. Filter by it with the **App** select on the Overview and the Prompts page. A prompt whose tool doesn't say shows the tool alone: GitPrompt never guesses.

| App | Captured? | How it is told apart |
|---|---|---|
| Claude Desktop, Code tab | Yes, by the Claude Code connector (Desktop runs the hooks in `~/.claude/settings.json`) | Telemetry layer: `service.name` `claude-code-desktop` (or `app.entrypoint` `claude-desktop`). Without telemetry: no app shown |
| Claude Desktop, chat | **No**: no hook, telemetry or local API exists, and GitPrompt never reads the app's own storage | |
| Claude Code CLI, VS Code extension, `claude -p` / Agent SDK | Yes | Telemetry `app.entrypoint`: `cli`, `claude-vscode`, `sdk-*` |
| Codex desktop app, CLI, IDE extension | Yes, by the Codex connector (they share `~/.codex/hooks.json`) | Not possible: no hook field names the client, so Codex prompts show no app |
| Cursor (the app's agent) | Yes, by the Cursor connector | Shown as Cursor; its CLI can't be told apart (and its prompt hooks are unreliable) |

The connector cards list, under **Where your prompts come from**, the apps each connector captures and the ones it can't, with why. A prompt sent from a git worktree (Claude Desktop, Claude Code and the Codex app can run sessions in one) belongs to its main repository's project; the worktree and its branch are kept. A Cursor window with several folders uses the first one as the prompt's folder and keeps the others.

**Check it live:** type one prompt in each app, then run `uv run gitprompt check` (or open **Recent captures** at the top of the Connectors page). For each tool it says whether each layer is connected and lists, per app, the newest prompt: when, project and branch, session, model, how many prompts, and what the app rules read (`app.entrypoint=…`). No row means nothing arrived; "surface unknown" means the tool doesn't say where it was typed.

### Adding or fixing a connector

Connectors are YAML files in `src/gitprompt/sources/bundled/`. To patch one without a release, copy it to `~/.gitprompt/sources/`, edit it, then use **Re-map stored events** on its card. A spec that adds a layer to another tool names it with `part_of`, plus a short `layer` name and a one-line `summary` ([ADR-0015](docs/adr/0015-connector-layers-and-states.md)). It declares what it yields in `capabilities`, and a tool that sends no prompt id sets `join: session` ([ADR-0017](docs/adr/0017-connector-capabilities.md)). `assistant` names who answers in the tool, `after_connect` says what the user still does in the tool once connected, and `tool_files` finds the files a tool names inside a text input such as a patch ([ADR-0019](docs/adr/0019-codex-connector.md)). `usage` names the events that report model calls and where their tokens are, and `capability_since` the tool version that adds a capability ([ADR-0020](docs/adr/0020-model-call-usage-and-version-capabilities.md)). `surface` rules say which app a prompt was typed in, `captures` and `cannot_capture` which apps the connector reaches; `event_names` maps a tool's own event names to GitPrompt's, `answer` says where the final answer is, and `discard` drops payload keys before storing ([ADR-0022](docs/adr/0022-surfaces-and-desktop-apps.md)).

A spec file that can't be read is listed at the top of the Connectors page and in Settings with its error; it is skipped, every other connector (and capture) keeps working, and a broken copy of a bundled spec falls back to the bundled one.

A connector is done when its conformance fixtures pass: synthetic payloads from the tool's documented examples in `tests/sources/fixtures/<id>/`, each with the fields, origin and turn it must map to (`tests/sources/test_conformance.py`).

## Keeping the database small

Telemetry start-up events (`hook_registered`, `plugin_loaded`, `hook_execution_*`, ...) are no longer stored: the spec lists them under `drop_events`, and the Fields page shows them as not stored. To remove those stored before, and optionally old raw events:

```bash
uv run gitprompt compact                          # dry run: rows and MB per event, nothing changes
uv run gitprompt compact --apply                  # delete them, then VACUUM
uv run gitprompt compact --events-older-than 30   # also raw events older than 30 days
```

Prompts and your notes are never removed. Removing the dropped events changes nothing on any page. Expired events take their turn details with them (tools, usage, timeline) for prompts older than the cutoff. Compact can run while the daemon is running.

The same clean-up, a persistent event retention and deleting events by source or date are on the **Settings** page; each shows a dry run first and only deletes what you confirm.

## Settings

**Settings** (`/settings`) groups everything you can configure: General (theme, default time range, week start), Connectors, Capture (event retention), Privacy (hide detected secrets on arrival), Data (database size and location, compact, delete events, backups), Security (rotate the receiver token, reconnecting each connected tool after you review its diff) and the preferences kept in this browser.

Preferences are stored in `~/.gitprompt/preferences.json` (written atomically, mode 0600). Each value comes from, in order: an environment variable, the file, the default. The page shows where each value comes from; one set by an environment variable is read-only there.

| Key in `preferences.json` | Environment variable | Default | Effect |
|---|---|---|---|
| `default_range` (`24h`, `7d`, `30d`, `all`) | `GITPROMPT_DEFAULT_RANGE` | `7d` | Window Overview, Sessions and Projects open with |
| `week_start` (`monday`, `sunday`) | `GITPROMPT_WEEK_START` | `monday` | First row of the weekday heatmap |
| `event_retention_days` (1–3650 or `null`) | `GITPROMPT_EVENT_RETENTION_DAYS` | `null` (keep) | Raw events older than this are deleted at start-up and every 6 hours |
| `hide_secrets_on_ingest` | `GITPROMPT_HIDE_SECRETS_ON_INGEST` | `false` | Replace detected secrets before a payload is stored |

All of them apply without a restart. `GITPROMPT_HOME`, `GITPROMPT_PORT` and `GITPROMPT_LOG_LEVEL` stay environment-only ([ADR-0013](docs/adr/0013-preferences.md)). Theme stays in the browser (`localStorage`).
## The prompt index

Filters, search and statistics run in SQL on a prompt index: one row per prompt with the values GitPrompt derives (origin, typed text, local day and hour, usage per model, tools, outcome), plus a full-text index of the typed text. It is a cache of the stored prompts and events: it updates itself as events arrive and rebuilds itself when the rules or the code that compute it change (a running daemon reads its code once: restart it after an update to see new derived values). To rebuild it by hand (for example after restoring a backup):

```bash
uv run gitprompt reindex
```

Day and hour buckets use this machine's timezone; changing the timezone rebuilds the index on next use.

## Layout

```
src/gitprompt/            FastAPI daemon, one package per domain (see PLAN.md §5)
  core/                   shared kernel: settings, preferences, domain errors, atomic writes, JSON payload helpers, git facts
  sources/                tool specs, probing, connect/disconnect; bundled/ holds the YAML specs
  capture/                ingest pipeline, the OTLP decoder, compact (dropped events and retention), the retention pass
  storage/                SQLite store, the rebuildable prompt index (FTS5) and statistics over it
  insights/               values derived on read, prompt origin, the indexer, secret redaction, field catalog
  export/                 exports: one redaction pass, report builders, one renderer per format
  api/                    FastAPI app, dependencies, live-update bus, routes/
  cli.py                  `gitprompt serve`, `gitprompt compact`, `gitprompt reindex`, `gitprompt check`
web/src/lib/              SvelteKit dashboard (built into src/gitprompt/dashboard)
  design/                 design tokens (tokens.css: primitives, semantic, scales) and base styles
  ui/                     generic components (Button, Badge, Panel, Tabs...); ui/charts/ for charts
  insights/               stories computed from prompts (sessions, projects, timelines), unit tested
  settings/               Settings page sections (one component per section), preference cache
  api/ layout/ prompts/ sessions/ projects/ sources/ shared/ testing/
tests/                    pytest, mirroring src/ (temp dirs only, never touches real tool configs)
```
