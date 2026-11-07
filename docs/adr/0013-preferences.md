# 0013 — User preferences: `preferences.json`, env > file > defaults

**Status:** accepted (implemented)
**Builds on:** [0011](0011-drop-noise-events.md) (retention was left as plain functions for a future setting)

## Context
Until now every setting was an environment variable read at start-up (`core/config.py`: `GITPROMPT_HOME`, `_PORT`, `_LOG_LEVEL`), and everything else was a constant in code. The Settings page (plan E1) needs values the user can change from the dashboard, without losing the ability to pin them from the environment (scripts, dotfiles, a managed machine).

## Decision
1. **One Pydantic model, `core.preferences.Preferences`**, stored as `~/.gitprompt/preferences.json`. Unknown keys and invalid values are refused (HTTP 422) and nothing is written. Writes are atomic (temp file + rename, mode 0600, `core/files.atomic_write`, now shared with the installer).
2. **Precedence env > file > default.** Every field gets an environment variable for free: `GITPROMPT_<KEY>`. A value set there is *locked*: the API reports `source: "env"`, the page shows it read-only with the variable to unset, and PUT refuses it (409). Every value reports where it came from.
3. **Re-read on use.** The file and env are read on every request (like source specs), so a change applies at once. A field that only takes effect at start-up goes in `APPLIES_AT_START`; PUT then answers `restart_required: true`. None needs it today.
4. **A broken file never stops capture.** The receiver and the retention pass use `effective()`, which falls back to the defaults and logs; the Settings page shows the error. PUT refuses to overwrite a file it can't parse, so hand edits are never lost.
5. **Env-only stays env-only:** `GITPROMPT_HOME` (the file lives under it), `GITPROMPT_PORT` (every connected hook embeds the receiver URL, so a port change would silently break capture until each source is reconnected) and `GITPROMPT_LOG_LEVEL`. The page shows the folder and receiver read-only.
6. **Browser-only preferences stay in `localStorage`** (theme), and are listed in one registry, `web/src/lib/settings/browser.ts`, that the page renders with a reset per entry.

### Which constants became settings
| Constant | Decision | Why |
|---|---|---|
| Default time range (`DEFAULT_RANGE`, 7d) | **setting** `default_range` | Users who look at a month at a time should not re-pick it on every page |
| Week start (Monday-first heatmap) | **setting** `week_start` | Varies by country; one line of display logic |
| Event retention (ADR-0011 `--events-older-than`) | **setting** `event_retention_days` | Was only a manual flag; a persistent value is what keeps the DB small |
| Hide secrets on ingest | **setting** `hide_secrets_on_ingest` | `insights.redaction.redact` already existed per prompt; applying it before storage is one line |
| Time zone | not a setting | The browser's zone is correct for a local tool; storage is UTC |
| `CAP` (1,000 prompts in the browser) | not a setting | An implementation limit F1 removes by moving aggregation server-side |
| `RUNNING_WINDOW_MS` (1 h "running" outcome) | not a setting | A heuristic, not a preference; users would not know what to set |
| `clip` length (10,000 chars in stored events) | not a setting | Storage detail; changing it only affects future events and nothing on the pages |

### Destructive actions
Compact, delete events (by source and/or before a day) and token rotation all follow the Connect pattern: the endpoint is a dry run by default, the page shows exactly what would change (rows and size per event kind, or each tool's settings diff), and only a second, explicit call applies it. "Delete everything" is not offered. Token rotation previews the diffs with a placeholder token and is refused when a connected source can't be reconnected (blocked by policy, unreadable config), since that source would silently stop capturing.

## Consequences
+ Adding a preference is one model field plus one row in a Settings section; its env var, validation, source reporting and lock come for free.
+ Retention now runs while the daemon is up (start-up, then every 6 hours), reading the preference each pass.
− Hiding secrets on ingest is irreversible for what arrives while it is on (raw is no longer verbatim for those payloads). It is off by default and explained on the page.
− Running tool sessions may keep the old token after a rotation until restarted; the page says so.
− A value set in both the file and the env is shown as env; the file value comes back if the variable is unset.

## Alternatives considered
- **Everything in `localStorage`:** no env lock, invisible to the daemon (retention, ingest), lost per browser.
- **TOML/YAML config file:** JSON matches what the API returns and needs no new dependency.
- **A preferences table in SQLite:** the DB is data that compact/delete act on; config should survive deleting it.
