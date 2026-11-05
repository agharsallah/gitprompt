# 0008 — Universal capture through documented interfaces and declarative source specs

**Status:** accepted (iteration 1 implemented); amended by [0009](0009-metadata-events-telemetry-derived-on-read.md)
**Supersedes:** [0004](0004-single-hook-gateway-dialects.md), [0005](0005-generic-readers-thin-connectors.md), [0007](0007-browser-site-adapters.md)
**Amends:** [0002](0002-hooks-signal-transcripts-truth.md) (transcripts become an optional add-on), [0003](0003-raw-event-inbox.md) (raw payload kept in the prompt row instead of a separate inbox)

## Context
v1 combined hooks with reading each tool's internal history files (JSONL transcripts, SQLite stores, VS Code mutation logs). Those formats are undocumented, change with tool releases, and sometimes are encrypted. They were most of the planned code, most of the expected maintenance, and the riskiest part with respect to the tools' terms (reading private application data).

Meanwhile, most CLI/IDE tools converged on Claude Code's hook shape (`session_id`, `prompt`, `cwd`, `transcript_path`, `hook_event_name`), and every web chat is a text box plus Enter or a Send button. We want:
- one generic mechanism for all CLI/IDE tools and one for all web chats;
- tool updates that need no new release, ideally no change at all;
- strict compliance with each tool's terms and the user's organization policy.

## Decision
1. **Capture only through documented, user-facing extension points.** For CLI/IDE tools: prompt-submit hooks in the user's own settings file. For the web (later): the text the user types into a page they enabled. No traffic interception, credentials, private file formats or automation (PLAN §4).
2. **A source is a YAML spec, not code** (`src/gitprompt/sources/*.yaml`):
   `detect` (paths), `version_cmd`, `install` (target file + JSON to merge, with `{{receiver}}`, `{{source}}`, `{{token}}`), `accept_events`, `map` (field → candidate paths), `policy` (settings files, `block_if`, `warn_if_set`) and `terms` (shown before consent).
   Shared fallback paths live in `sources/_defaults.yaml`. Specs are re-read on every use; `~/.gitprompt/sources/<name>.yaml` overrides a bundled one.
3. **One receiver** `POST /ingest/{source}`: token check → mapping → git context → store → live push. It always answers with an empty 2xx, which hook-capable tools treat as "no output" (no blocking, no added context).
4. **Raw payloads are always stored** next to the mapped fields. `remap` rebuilds fields after a spec change. Events with no prompt are counted per source as a drift signal.
5. **One generic config installer:** merge into the target JSON, recognise our entries by the receiver URL they contain (no extra keys), parse-or-abort, show the diff, back up, write atomically, keep the file mode; uninstall removes only our entries.
6. **Organization policy wins:** if the spec's policy files disable user hooks, the source is shown as blocked and install is refused.
7. **Web capture (iteration 3)** uses one generic content script (focused text box on Enter/Send, on user-enabled sites) instead of per-site request adapters. Per-site selector overrides are allowed as data.

## Consequences
+ Adding a hook-capable tool is a YAML file; tools that copy Claude Code's shape may need only an `install` block.
+ Most tool updates need nothing (new fields are kept and displayed; common renames are covered), the rest are a data edit plus remap, without a release.
+ Much smaller codebase: no readers, dialect code, inbox worker or per-site adapters in iteration 1.
+ Compliance is structural: the system has no code path that touches credentials, traffic or private files.
− No history before connection. Backfill returns as an optional, per-tool add-on (the only part that tracks private formats).
− Tools whose hooks don't include the prompt text (Antigravity's `PreInvocation`) need that add-on to be covered.
− Hook APIs that are preview or experimental (VS Code Copilot, Gemini CLI) can change; the drift signal and spec overrides limit the impact.
− The receiver token is stored in the tool's user settings file (user-only permissions, valid only against 127.0.0.1).

## Alternatives considered
- **Local HTTPS proxy / MITM of AI API traffic:** covers many tools generically, but needs a trusted root certificate, sees far more than prompts, doesn't work for Cursor or subscription web chats, and conflicts with the tools' terms. Rejected.
- **OpenTelemetry receiver** (Claude Code, Gemini CLI, Codex can export user prompts): documented and vendor-maintained, but lacks `cwd`, needs env/config changes per tool and isn't available for Cursor. Kept as an optional add-on.
- **Terminal recording (PTY):** generic for CLIs, but misses IDE surfaces and has to parse TUI escape sequences. Rejected.
- **Keep v1 (hooks + transcript readers by default):** most complete history, highest maintenance and compliance risk. Moved to an optional add-on.
