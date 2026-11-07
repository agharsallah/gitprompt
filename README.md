# GitPrompt

Keep every prompt you write to an AI tool, with its metadata, **locally**.

Iteration 1 supports **Claude Code** (terminal, IDE extensions, desktop app). See [PLAN.md](PLAN.md) for the design and roadmap, and [ADR-0008](docs/adr/0008-universal-capture-declarative-sources.md) for why it works this way.

## Quick start

```bash
just setup
just serve          # opens on http://127.0.0.1:7878
```

1. Open **Sources** → Claude Code → **Connect**. Review the exact change to `~/.claude/settings.json` and confirm (a backup goes to `~/.gitprompt/backups/`).
2. Optionally connect **Claude Code telemetry** too, for model, tokens and cost (applies to sessions started afterwards).
3. Type a prompt in any Claude Code session. It appears live under **Prompts**, on its session's rail. Select it to see the turn as a story: what you asked, a timeline of model and tool calls, the answer, then tabs for activity (tools, files, commands), usage (tokens, cache, models), the repository (state at send time, commits afterwards), the session and every event. Star, rate, tag and annotate prompts there. Prompts you didn't type (task notifications, agent messages, subagent and programmatic runs) are muted and labelled with their origin, and context the tool added (an IDE selection, a system reminder) folds into a note under what you typed.
4. **Overview** shows where your prompts went (projects over time), when you prompt, spend and tokens, what the agent did, how you write, and what needs a look (secrets, failed turns). **Sessions** and **Projects** group the same prompts by conversation and by repository. Each takes a time range.
5. **Fields** lists every captured field next to what the tool documents, to decide what to build on. **Design system** shows the tokens and components the dashboard is built from.
6. **Disconnect** removes only what GitPrompt added. After an upgrade, a connected source may offer **Update connection** (with the diff first).

## How it stays within the tools' terms

GitPrompt uses only Claude Code's documented [hooks](https://code.claude.com/docs/en/hooks) and, if you connect it, its [OpenTelemetry export](https://code.claude.com/docs/en/monitoring-usage), in your user settings and pointed at this machine. It subscribes only to events that report what happened, never to decision points, and answers each hook with an empty response, so nothing is blocked or changed. It never reads credentials, intercepts traffic, reads Claude Code's private files, or sends anything off your machine. If your organization turns hooks off, GitPrompt shows that and doesn't install anything. Full rules: [PLAN.md §4](PLAN.md#4-terms-of-service-and-policy-guardrails).

## Adding or fixing a source

Sources are YAML files in `src/gitprompt/sources/bundled/`. To patch one without a release, copy it to `~/.gitprompt/sources/`, edit it, then use **Re-map stored events** on the Sources page.

## Layout

```
src/gitprompt/            FastAPI daemon, one package per domain (see PLAN.md §5)
  core/                   shared kernel: settings, domain errors, JSON payload helpers, git facts
  sources/                tool specs, probing, connect/disconnect; bundled/ holds the YAML specs
  capture/                ingest pipeline and the OTLP decoder
  storage/                SQLite store
  insights/               values derived on read, prompt origin, secret redaction, field catalog
  api/                    FastAPI app, dependencies, live-update bus, routes/
  cli.py                  `gitprompt serve`
web/src/lib/              SvelteKit dashboard (built into src/gitprompt/dashboard)
  design/                 design tokens (tokens.css: primitives, semantic, scales) and base styles
  ui/                     generic components (Button, Badge, Panel, Tabs...); ui/charts/ for charts
  insights/               stories computed from prompts (sessions, projects, timelines), unit tested
  api/ layout/ prompts/ sessions/ projects/ sources/ shared/ testing/
tests/                    pytest, mirroring src/ (temp dirs only, never touches real tool configs)
```
