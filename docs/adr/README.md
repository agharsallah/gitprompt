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
| [0015](0015-connector-layers-and-states.md) | Connectors: tools with layers (`part_of`), raw facts in `/api/sources`, one derived state per connector | accepted |
