#!/usr/bin/env node
// The code-quality dashboard (`just quality`): opens it, then builds the reports while it watches.
//
//   just quality                    the checks; the dashboard opens at once and fills in live
//   just quality measure            the measurements (timings that run alone, minutes each)
//   just quality all                checks and measurements
//   just quality web                one area's checks just quality backend/lint   one report
//   just quality lint               a report name in every area
//   just quality --view             open the dashboard as last built, build nothing
//   just quality --list             the areas and reports
//   just quality --no-open ...      build without opening a browser (CI, scripts)
//   just quality --jobs=1 ...       reports at once (default: half the cores, at most 4)
//   just quality --force ...        rebuild reports whose inputs did not change too
//
// Adding a report: tools/quality/README.md.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';

import { AREAS } from './areas.mjs';
import { takeLock } from './lib/lock.mjs';
import { OUT, PATHS, fromRepo } from './lib/paths.mjs';
import { duration } from './lib/progress.mjs';
import { GROUPS, discover, select } from './lib/registry.mjs';
import { installHub, run } from './lib/runner.mjs';
import { DEFAULT_JOBS } from './lib/scheduler.mjs';

const args = process.argv.slice(2);
const options = args.filter((arg) => arg.startsWith('--'));
const flags = new Set(options.filter((arg) => !arg.includes('=')));
const selectors = args.filter((arg) => !arg.startsWith('--'));
const known = new Set(['--view', '--list', '--no-open', '--force']);
const unknown = options.filter((option) => !known.has(option) && !option.startsWith('--jobs='));
if (unknown.length > 0) fail(`unknown option ${unknown.join(', ')} (options: ${[...known, '--jobs=N'].join(', ')})`);
const jobs = jobsOption(options.find((option) => option.startsWith('--jobs=')));

const reports = await discover();

if (flags.has('--list')) {
	for (const area of AREAS) {
		console.log(`${area.title}  ${area.description}`);
		for (const report of reports.filter((r) => r.area === area.id)) {
			const kind = report.optIn ? ' (opt-in: only when named)' : report.measure ? ' (measurement: `measure`, `all` or by name)' : '';
			console.log(`  ${report.id.padEnd(22)} ${report.title}: ${report.description}${kind}`);
		}
	}
	console.log('Groups');
	for (const [name, what] of Object.entries(GROUPS)) console.log(`  ${name.padEnd(22)} ${what}`);
	process.exit(0);
}

if (flags.has('--view')) {
	await installHub();
	if (!existsSync(PATHS.status)) console.log('no report built yet: the dashboard opens empty; run `just quality` to build them');
	openPage(PATHS.index);
	process.exit(0);
}

let selected;
try {
	selected = select(reports, selectors);
} catch (error) {
	fail(error.message);
}

mkdirSync(OUT, { recursive: true });
const lock = takeLock(PATHS.lock, args);
if (lock.held) {
	const { pid, started_at: since, args: theirs = [] } = lock.held;
	const what = pid ? `pid ${pid}, \`just quality ${theirs.join(' ')}\`, started ${duration(Date.now() - since)} ago` : 'starting now';
	fail(`another build is running (${what}): two at once would break each other. Wait for it (the dashboard shows it), or stop it, then run again.`);
}
process.on('exit', () => lock.release());

const atOnce = Math.min(jobs, selected.length);
console.log(`\n  GitPrompt quality · ${selected.length} of ${reports.length} reports, ${atOnce} at a time · ${fromRepo(PATHS.index)}\n`);
const ok = await run(reports, selected, { jobs: atOnce, force: flags.has('--force'), onStarted: () => flags.has('--no-open') || openPage(PATHS.index) });
process.exit(ok ? 0 : 1);

function openPage(path) {
	const [command, ...rest] = process.platform === 'darwin' ? ['open'] : process.platform === 'win32' ? ['cmd', '/c', 'start', '""'] : ['xdg-open'];
	spawn(command, [...rest, path], { stdio: 'ignore', detached: true }).unref();
}

/** `--jobs=N` → N (at least 1); absent → DEFAULT_JOBS. */
function jobsOption(option) {
	if (!option) return DEFAULT_JOBS;
	const count = Number(option.slice('--jobs='.length));
	if (!Number.isInteger(count) || count < 1) fail(`${option}: give a whole number of reports at once, 1 or more`);
	return count;
}

function fail(message) {
	console.error(`quality: ${message}`);
	process.exit(2);
}
