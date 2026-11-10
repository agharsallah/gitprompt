# Architecture Decision Records

Format: [MADR](https://adr.github.io/madr/)-lite: Context · Decision · Consequences · Alternatives considered.
Status values: `proposed` → `accepted` → (`superseded by NNNN`). 0008 is the v2 decision; see PLAN.md §1 for what changed.

| # | Title | Status |
|---|---|---|
| [0001](0001-local-first-daemon-pluggable-storage.md) | Local-first single daemon, SQLite by default, Postgres (local or cloud) optional | accepted (SQLite); Postgres deferred |
| [0002](0002-hooks-signal-transcripts-truth.md) | Hooks are the signal, transcripts are the truth | amended by 0008 |
| [0003](0003-raw-event-inbox.md) | Append-only raw event inbox, replayable normalization | amended by 0008 |
| [0004](0004-single-hook-gateway-dialects.md) | One hook gateway, dialects as data | superseded by 0008 |
| [0005](0005-generic-readers-thin-connectors.md) | Generic readers, thin declarative connectors | superseded by 0008 |
| [0006](0006-contracts-generated.md) | Pydantic as single source of truth for contracts | deferred |
| [0007](0007-browser-site-adapters.md) | Browser capture via one engine + declarative site adapters | superseded by 0008 |
| [0008](0008-universal-capture-declarative-sources.md) | Universal capture through documented interfaces and declarative source specs | accepted |
| [0009](0009-metadata-events-telemetry-derived-on-read.md) | Rich metadata: all observational events, opt-in telemetry, derived on read | accepted |
| [0010](0010-prompt-origin.md) | Prompt origin and typed text, from spec rules, derived on read | accepted |
| [0011](0011-drop-noise-events.md) | Drop telemetry lifecycle noise; `gitprompt compact` and event retention | accepted |
| [0012](0012-prompt-index-and-stats.md) | A rebuildable prompt index for filters, FTS search and `/api/stats` | accepted |
| [0013](0013-preferences.md) | User preferences: `preferences.json`, env > file > defaults, and which constants became settings | accepted |
| [0014](0014-insights-stats.md) | Insights dashboard: more of the index, split groupings, per-session telemetry | accepted |
| [0015](0015-connector-layers-and-states.md) | Connectors: tools with layers (`part_of`), raw facts in `/api/sources`, one derived state per connector | accepted |
| [0016](0016-prompt-stories.md) | Prompt stories: phases, lineage by text similarity, search snippets, and the stories API | accepted |
| [0017](0017-connector-capabilities.md) | Connector capabilities, a `join: session` fallback for tools without a prompt id, isolated spec loading, turning a connector off | accepted |
| [0018](0018-export.md) | Export: one redaction pass, a neutral report, server-side renderers, files only | accepted |
| [0019](0019-codex-connector.md) | Codex connector: hooks only, files from patches, assistant and after-connect steps | accepted |
| [0020](0020-model-call-usage-and-version-capabilities.md) | Model-call usage from a spec `usage` block, capabilities by tool version, interrupted turns | accepted |
| [0021](0021-turn-facts-and-field-coverage.md) | Turn facts computed once, `ignored` payload keys and a field-coverage test, a display contract, filters by tool, model and outcome | accepted |
| [0022](0022-surfaces-and-desktop-apps.md) | Surfaces (where a prompt was typed) as spec rules, desktop apps, a Cursor connector, worktrees resolved to their repository, `gitprompt check` | accepted |
| [0023](0023-rust-backend.md) | The backend is Rust (one crate per domain, wire compatible, `event_stats`, dashboard embedded, offline spool); the Python daemon is deprecated in `legacy/python` | accepted |
