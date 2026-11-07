# 0018 — Export: one redaction pass, a neutral report, server-side renderers, files only

**Status:** accepted (implemented)
**Amends:** [0013](0013-preferences.md) (export options are per export, not preferences), [0016](0016-prompt-stories.md) (stories are exported from the same JSON)

## Context
Plan epic E5 asks to turn a story, the Overview or a prompt selection into a file the user can send: Markdown, a self-contained HTML file, CSV/JSON of the rows, and a single chart as PNG/SVG. Rules that bind it: nothing leaves the machine (plan §0.1, §7 question 3: sharing stays file-only, no hosted links or upload); the exported HTML makes no network request; secrets, email addresses and home paths never appear in any format by default; the preview shows exactly what the file holds; options default to the most private choice.

## Decision
1. **Pipeline** (`src/gitprompt/export/`): `data ──redact──► clean data ──BUILDERS[kind]──► Report + rows ──FORMATS[format]──► text`. The API collects the data (`api/routes/export.py`: the story routes' own functions, `storage.stats.aggregate`, `timeline.prompts`); everything after that is pure.
2. **One redaction pass** (`export/redact.py`), applied once to the collected JSON before any builder or format sees it, never per format:
   - **Secrets: always hidden** with `insights.redaction.SECRETS` (`[hidden <kind>]`). Not an option: a sent file can be forwarded, and a leaked key can't be taken back. A clipped excerpt that ends in a cut token loses that token (`CUT`): half a key matches no pattern.
   - `prompt_text` (default **off**): drops keys holding typed or system text (`TEXT_KEYS`: text, typed_text, excerpt, label, title, snippet, diff, the `q` filter). Off, an export has figures, outcomes, files and commits only.
   - `relative_paths` (default **on**): inside a project folder (values under `repo`/`git_root`, plus the Overview's project keys) paths become relative and the folder itself its name; the user's home becomes `~`, and so does any `/Users/<name>` or `/home/<name>`; a home folder written as one path component (`-Users-<name>-…`, as tools name per-project folders) is dropped.
   - `drop_identity` (default **on**): drops `user.*` and `organization.*` telemetry resource attributes, commit authors and id keys, and masks every email address (`[hidden email]`) in every string.
   - `drop_raw` (default **on**): drops stored payloads (`raw`, `fields`, `context`, `events`); only a prompt selection collects them, and only when this is off.
3. **A neutral report** (`export/report.py`): sections of figures, charts (`bars`, `columns`), paragraphs, quotes (voice `you` or `system`, so system text is never shown as typed, ADR-0010), item lists and tables, plus notes on what the export includes and leaves out, printed in the file itself. One builder per kind (`export/build.py`: session, project, overview, prompts) returns the report and its rows (chapters and markers; project prompts with their phase; long-format statistics buckets; prompt rows).
4. **Rendering happens in Python** with the standard library (`html.escape`, `csv`, `json`): no template engine, no new dependency. One module per format registers itself (`formats.register`): `markdown.py` (charts as tables; prompt text in a fence so its Markdown, links and images stay inert), `html.py`, `tabular.py` (CSV with formula cells quoted, JSON with the options), `charts.py` (`svg`: one chart, standalone). Adding a format touches no other module.
5. **Self-contained HTML**: inline CSS and inline SVG charts drawn from the report (no namespace URL needed inline), no script, font, image or link; colours are CSS variables switched by `prefers-color-scheme`, and `@media print` forces the light palette and hides the "Numbers" disclosures. The file carries its own `Content-Security-Policy` (`default-src 'none'; style-src 'unsafe-inline'; img-src data:`), so even a value that slipped through can't load anything; the response sends the same header.
6. **API**: `GET /api/export/{kind}` (`session`, `project` with `id`; `overview`, `prompts` with the prompt filters) takes `format` (`md`, `html`, `csv`, `json`, `svg` + `chart`), the four options (most private by default) and `download` (attachment with a `gitprompt-<kind>-<title>-<day>.<ext>` name). `GET /api/export/{kind}/charts` lists the charts with data. A prompt selection exports the newest 2,000 prompts the filters keep (the total is stated). Read-only, nothing stored.
7. **Preview = file** (`web/src/lib/export/`): one Export button per page (session, project, Overview with its filters, Prompts with its server-side filters) opens a dialog that renders the daemon's response itself: the HTML in a sandboxed `iframe srcdoc`, Markdown/CSV/JSON as text, a chart as an image. **Save file** saves those previewed bytes (no second request). A single chart's SVG is the daemon's; its PNG is drawn from that SVG on a canvas in the browser (2×). Options reset to the private defaults each time the dialog opens; nothing is remembered, so no new preference (Settings › Privacy lists the defaults and points here).

## Verification
- `tests/export/test_export.py`: every format of a synthetic session with a planted Anthropic key, email, home paths, account ids and a commit author, under three option sets: none of them appears; prompt Markdown stays inert, HTML is escaped, CSV formulas are quoted, a new format registers without other changes.
- `tests/api/test_export_routes.py`: through ingest (hooks + OTLP with `user.email`/`organization.id` resource attributes, a git repository inside the test home), every kind × every format with defaults leaves out secrets, emails, home paths, authors and prompt text; with prompt text on, secrets, emails and home paths stay hidden; each option puts back only what it hides; the HTML has no `script`/`link`/`img`/`iframe`/`base`, no `src`/`href` attribute, no `http(s)://`, no `url(` or `@import`, and carries the CSP.
- On a copy of the local database, each kind's HTML (with and without prompt text, light, dark, 390 px, print) was opened from `file://` in Chrome with the network offline and every request intercepted: 0 requests besides the file, no horizontal scroll (a control page with an external stylesheet and image recorded 2). Exports render in 40–160 ms.

## Consequences
+ One place decides what leaves in a file; a format can't forget a rule.
+ The preview is the file, byte for byte.
− Dropping by key name is coarse: a future field holding typed text under a new key must be added to `TEXT_KEYS` (the planted-text tests would catch it only if their fixtures include it).
− The home-folder rules can rewrite a non-path string that looks like `/Users/<name>`; acceptable for a privacy default.
− The Prompts page's quick filters (`?show=`) are client-side and not part of a prompt-selection export.
− Charts in the file are the report's own (activity, origins, commands, models, cost, tool kinds, outcomes, phases), not every Overview panel.

## Alternatives considered
- **Render in the browser** from the dashboard's components: the preview would be a different renderer from Markdown/CSV and the redaction would live in TypeScript next to a Python copy.
- **Jinja2 templates**: not a dependency of the daemon (FastAPI doesn't bring it); escaping is easy to get right with `html.escape` on a small fixed structure.
- **Redact per format**: one rule forgotten in one format is a leak; rejected.
- **Hosted share links or upload**: out of scope by the local-only rule (plan §7 question 3).
- **PNG on the server**: needs an image library; the browser rasterizes the same SVG.
