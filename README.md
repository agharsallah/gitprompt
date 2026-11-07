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
3. Type a prompt in any Claude Code session. It appears live under **Prompts**. The detail shows the turn (response, tools, files, commands), usage, prompt stats, git state, commits made afterwards, the session and every event. Star, rate, tag and annotate prompts there.
4. **Fields** lists every captured field next to what the tool documents, to decide what to build on.
5. **Disconnect** removes only what GitPrompt added. After an upgrade, a connected source may offer **Update connection** (with the diff first).

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
  insights/               values derived on read, secret redaction, field catalog
  api/                    FastAPI app, dependencies, live-update bus, routes/
  cli.py                  `gitprompt serve`
web/src/lib/              SvelteKit dashboard (built into src/gitprompt/dashboard)
  api/ prompts/ sources/ shared/
tests/                    pytest, mirroring src/ (temp dirs only, never touches real tool configs)
```
