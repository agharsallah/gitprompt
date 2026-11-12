# Code-quality dashboard

One page for every quality report on the backend and the web dashboard.

```bash
just quality                  # opens the dashboard at once, then builds every report whose inputs changed
just quality web              # one area            just quality backend/lint   one report
just quality lint             # a report name, in every area (backend/lint and web/lint)
just quality --view           # open the dashboard as last built, build nothing
just quality --list           # areas and reports
just quality --no-open ...    # build without opening a browser (CI, scripts)
just quality --jobs=1 ...     # reports at once (default: half the cores, at most 4)
just quality --force ...      # rebuild reports whose inputs did not change too
```

The dashboard is `target/quality/index.html` (git-ignored). It works straight from disk, no server:
the build writes scripts the page loads (`status.js`, `data/…`), and the page reads `status.js`
again every second while a build runs, so you watch the build: which reports run, the command each
one runs, what it is doing (crates compiled, the test binary running and tests passed so far, test
files done), its last output lines, its time against the last run, what it waits for, and each
result as it lands. The terminal shows the same in a live block, one line per running report. A
failed report does not stop the others (exit status 1 at the end); its page keeps the previous
result under the error, with a link to its log (`target/quality/logs/`).

## Only what changed

A report whose inputs did not change since its last successful run keeps its result: the build
marks it up to date (`=`) instead of running it, so after a web-only change only the web reports
run, and with nothing changed `just quality` takes a second. `--force` rebuilds anyway (to measure
the benchmark again on a quieter machine, say); a failed report always runs again.

A report's inputs (`lib/fingerprint.mjs`) are its area's (`areas.mjs`: `backend/` and the API types
file it is checked against; `web/`), its own `inputs` (Lighthouse adds `backend/` and the demo
data), its file, the dashboard's code (`lib/`, `areas.mjs`) and the `justfile`. They are hashed by
path and content as in the working tree (git's content ids, so staging or committing never changes
the hash, only an edit does); ignored files (`target/`, `node_modules/`, build output) never count. Things
outside the repository (a new Rust toolchain, an updated global tool) do not count either: use
`--force` after one.

## Side by side

Reports run in parallel (`--jobs`, default half the cores and at most 4: a Rust build alone uses
every core), the longest one (by its last run) first. Three rules keep that safe:

- A report waits for the ones in its `after` (when they are in the same build): the architecture
  page draws the coverage `backend/tests` writes.
- A step several reports need runs once (`lib/shared.mjs`): the SvelteKit sync, the web build,
  clippy, the daemon build, the demo daemon. Anything several reports write belongs there.
- A measurement runs last and alone (`ctx.alone`): the load benchmark and Lighthouse build what
  they need side by side with the rest, then measure one after the other once every other report
  is done, so their timings never measure a busy machine and they never hold up the others.

Cargo commands on the same `backend/target` profile still take turns on cargo's own lock (the
dashboard shows the wait); coverage builds into its own folder and the benchmark in release, so
those really run side by side. `just quality --jobs=1` runs one report at a time, as before.

## Reports

| Area | Report | What it runs | Headline |
|---|---|---|---|
| Backend | `backend/tests` | every Rust test under `cargo llvm-cov` (`just cov`) | lines covered (failing tests first) |
| Backend | `backend/architecture` | `just arch-report`: rules, layers, module matrix | modularity score |
| Backend | `backend/lint` | `cargo clippy` (JSON) and `cargo fmt --check` | findings |
| Backend | `backend/dependencies` | `cargo deny check`, `cargo machete` | findings |
| Backend | `backend/performance` | the load benchmark with its budgets (`just perf`), release build | slowest read p95 under load |
| Backend | `backend/complexity` | the architecture test's cognitive complexity of every function (`COMPLEXITY_JSON`), file sizes against `MAX_LINES`, clippy's nesting, argument and length limits, the exemptions (`COMPLEX`, `#[expect]`) | functions, files and blocks over a limit |
| Web | `web/tests` | `vitest run --coverage` | lines covered |
| Web | `web/lint` | ESLint, `svelte-check`, Prettier | findings |
| Web | `web/bundle` | `vite build`, then every file of `web/build` raw and gzipped | JavaScript gzipped |
| Web | `web/lighthouse` | Lighthouse on key pages of the built dashboard, served by a debug daemon with demo data | performance score |
| Web | `web/complexity` | ESLint's complexity rules (sonarjs cognitive complexity, function and file length at 0; depth, parameters, callbacks at their limits) | functions, files and blocks over a limit |

Both Complexity pages share one builder (`lib/complexity.mjs`) and read their limits from the
configs the gate uses (`rules.rs`, `clippy.toml`, `eslint.config.js`), so the page and the gate never
disagree.

The full run takes a few minutes (coverage, a release build for the benchmark, Lighthouse); `just
quality <report>` refreshes one in seconds to minutes and keeps the others. Unchanged reports are
kept as they are (above).

## How it is built

```
tools/quality/
  quality.mjs            the command: selection, then the runner
  areas.mjs              the areas (Backend, Web): one line each
  lib/
    registry.mjs         finds reports/<area>/NN-<name>.mjs; id <area>/<name>; its inputs
    fingerprint.mjs      hashes a report's inputs: is its last result still up to date?
    scheduler.mjs        which report runs when: places, `after`, longest first, measurements alone
    runner.mjs           runs them through the scheduler, writes progress and results as it goes
    context.mjs          what a report gets: exec (logged, live), step, once, alone, cleanup, readJson
    output.mjs           live output: whole lines, and what a cargo command is doing
    shared.mjs           steps several reports share, once per build: sync, web build, clippy, daemon
    spec.mjs             the page blocks and tone helpers
    status.mjs           status.js, state.json, data/<area>/<name>.js
    progress.mjs         the terminal progress
  hub/                   the page (index.html, app.js, build.js, render.js, style.css), copied to target/quality
  reports/<area>/NN-<name>.mjs
  test/                  node --test 'tools/quality/test/*.test.mjs'
```

A report never writes HTML: it returns its page as data, and the hub draws every report the same
way. Tools with their own HTML report (llvm-cov, vitest, Lighthouse, the architecture page) are
embedded in the page.

## Adding a report

One file: `reports/<area>/NN-<name>.mjs` (NN orders the sidebar). Nothing else changes.

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

- `ctx.exec(command, args, { cwd, env, label, allowFailure, activity, dataOnStdout })` runs a
  command; its output goes to the report's log and, line by line, to the live view; `label` to the
  progress line and the dashboard. It rejects on a non-zero exit unless `allowFailure` (for tools
  that exit 1 when they find something). `activity: cargoActivity()` (`lib/output.mjs`), or any
  `line => text | undefined`, says what the command is doing instead of its raw last line;
  `dataOnStdout` keeps JSON a report parses out of the live view. Commands of one report may run
  together (`Promise.all`) when they do not depend on each other.
- `after: ['backend/tests']` (next to `title`) waits for those reports when they are in the build.
- `inputs: ['tools/seed-demo.py']` (next to `title`): files or folders outside its area its result
  depends on; the area's own come with it.
- `ctx.alone(reason, fn)` runs `fn` once every other report is done, with nothing else running:
  for timings. Build everything `fn` needs before calling it.
- `ctx.step(text)` shows what the report is doing; `ctx.artifacts` is its own folder for files;
  `ctx.fromHub(path)` turns a file into a link the page can open or embed.
- `ctx.once(key, fn)` runs a step once per build, whichever report asks first; the others wait for
  it and say so. Put shared steps in `lib/shared.mjs` (`webSync`, `webBuild`, `clippy`,
  `daemonBinary`, `demoDaemon`). `ctx.cleanup(fn)` runs after the build.
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
