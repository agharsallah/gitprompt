# Code-quality dashboard

One page for every quality report on the backend and the web dashboard.

```bash
just quality                  # opens the dashboard at once, then builds every report while it fills in
just quality web              # one area            just quality backend/lint   one report
just quality lint             # a report name, in every area (backend/lint and web/lint)
just quality --view           # open the dashboard as last built, build nothing
just quality --list           # areas and reports
just quality --no-open ...    # build without opening a browser (CI, scripts)
```

The dashboard is `target/quality/index.html` (git-ignored). It works straight from disk, no server:
the build writes scripts the page loads (`status.js`, `data/…`), and the page reads `status.js`
again every second while a build runs, so you watch the build: which report runs, its current step
and elapsed time, each result as it lands. The terminal shows the same progress. A failed report
does not stop the others (exit status 1 at the end); its page keeps the previous result under the
error, with a link to its log (`target/quality/logs/`).

## Reports

| Area | Report | What it runs | Headline |
|---|---|---|---|
| Backend | `backend/tests` | every Rust test under `cargo llvm-cov` (`just cov`) | lines covered (failing tests first) |
| Backend | `backend/architecture` | `just arch-report`: rules, layers, module matrix | modularity score |
| Backend | `backend/lint` | `cargo clippy` (JSON) and `cargo fmt --check` | findings |
| Backend | `backend/dependencies` | `cargo deny check`, `cargo machete` | findings |
| Backend | `backend/performance` | the load benchmark with its budgets (`just perf`), release build | slowest read p95 under load |
| Web | `web/tests` | `vitest run --coverage` | lines covered |
| Web | `web/lint` | ESLint, `svelte-check`, Prettier | findings |
| Web | `web/bundle` | `vite build`, then every file of `web/build` raw and gzipped | JavaScript gzipped |
| Web | `web/lighthouse` | Lighthouse on key pages of the built dashboard, served by a debug daemon with demo data | performance score |

The full run takes a while (coverage, a release build for the benchmark, Lighthouse); `just quality
<report>` refreshes one in seconds to minutes and keeps the others.

## How it is built

```
tools/quality/
  quality.mjs            the command: selection, then the runner
  areas.mjs              the areas (Backend, Web): one line each
  lib/
    registry.mjs         finds reports/<area>/NN-<name>.mjs; id <area>/<name>
    runner.mjs           runs them in order, writes progress and results as it goes
    context.mjs          what a report gets: exec (logged), step, once, cleanup, readJson
    shared.mjs           steps several reports share, once per build: web build, daemon, demo daemon
    spec.mjs             the page blocks and tone helpers
    status.mjs           status.js, state.json, data/<area>/<name>.js
    progress.mjs         the terminal progress
  hub/                   the page (index.html, app.js, render.js, style.css), copied to target/quality
  reports/<area>/NN-<name>.mjs
```

A report never writes HTML: it returns its page as data, and the hub draws every report the same
way. Tools with their own HTML report (llvm-cov, vitest, Lighthouse, the architecture page) are
embedded in the page.

## Adding a report

One file: `reports/<area>/NN-<name>.mjs` (NN orders the run and the sidebar). Nothing else changes.

```js
import { join } from 'node:path';
import { kpis, table, note, embed, tone, toneZero } from '../../lib/spec.mjs';

export default {
	title: 'Audit',
	description: 'Known vulnerabilities in the web dependencies (pnpm audit).',
	needs: [{ command: 'pnpm', install: 'https://pnpm.io/installation' }], // skipped, with this hint, when missing
	async run(ctx) {
		const result = await ctx.exec('pnpm', ['--dir', 'web', 'audit', '--json'], { label: 'pnpm audit', allowFailure: true });
		const advisories = Object.values(JSON.parse(result.stdout).advisories || {});
		return {
			headline: { value: advisories.length, label: 'advisories', tone: toneZero(advisories.length) },
			summary: `${advisories.length} known vulnerabilities`,
			sections: [
				kpis([{ label: 'Advisories', value: advisories.length, tone: toneZero(advisories.length) }]),
				table('Advisories', [{ key: 'module', label: 'Package', mono: true }, { key: 'severity', label: 'Severity' }, { key: 'title', label: 'Title' }],
					advisories.map((a) => ({ module: a.module_name, severity: a.severity, title: { text: a.title, href: a.url } })))
			]
		};
	}
};
```

- `ctx.exec(command, args, { cwd, env, label, allowFailure })` runs a command; its output goes to
  the report's log, `label` to the progress line and the dashboard. It rejects on a non-zero exit
  unless `allowFailure` (for tools that exit 1 when they find something).
- `ctx.step(text)` shows what the report is doing; `ctx.artifacts` is its own folder for files;
  `ctx.fromHub(path)` turns a file into a link the page can open or embed.
- `ctx.once(key, fn)` runs a step once per build, whichever report asks first: put shared steps in
  `lib/shared.mjs` (`webBuild`, `daemonBinary`, `demoDaemon`). `ctx.cleanup(fn)` runs after the build.
- Blocks (`lib/spec.mjs`): `kpis`, `bars`, `table` (sortable, filterable when long), `checks`,
  `embed`, `links`, `note`. Tones: `good`, `warn`, `bad`, `neutral`; `tone`, `toneBelow`, `toneZero`
  pick one from a number.
- Keep the command a report runs in the justfile or package.json when people also run it alone, so
  the dashboard and the command line never disagree.

A new area is one line in `areas.mjs` and a folder in `reports/`.

## Ideas for more reports

- `web/audit`: `pnpm audit` (above), and outdated packages (`pnpm outdated --format json`).
- `backend/size`: the release binary and the desktop installer over time.
- `backend/build-time`: `cargo build --timings` (its own HTML report embeds as is).
- `backend/api`: endpoints and their snapshot tests (`tests/snapshots/`), and generated types in sync.
- `web/a11y`: axe-core over every page, beyond Lighthouse's sample.
- `history`: keep each build's headlines in `state.json` and draw the trend on every card.
