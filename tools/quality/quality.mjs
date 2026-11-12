#!/usr/bin/env node
// The code-quality dashboard (`just quality`): opens it, then builds the reports while it watches.
//
//   just quality                    every report; the dashboard opens at once and fills in live
//   just quality web                one area          just quality backend/lint   one report
//   just quality lint               a report name in every area
//   just quality --view             open the dashboard as last built, build nothing
//   just quality --list             the areas and reports
//   just quality --no-open ...      build without opening a browser (CI, scripts)
//
// Adding a report: tools/quality/README.md.
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';

import { AREAS } from './areas.mjs';
import { PATHS, fromRepo } from './lib/paths.mjs';
import { discover, select } from './lib/registry.mjs';
import { installHub, run } from './lib/runner.mjs';

const args = process.argv.slice(2);
const flags = new Set(args.filter((arg) => arg.startsWith('--')));
const selectors = args.filter((arg) => !arg.startsWith('--'));
const known = new Set(['--view', '--list', '--no-open']);
const unknown = [...flags].filter((flag) => !known.has(flag));
if (unknown.length > 0) fail(`unknown option ${unknown.join(', ')} (options: ${[...known].join(', ')})`);

const reports = await discover();

if (flags.has('--list')) {
	for (const area of AREAS) {
		console.log(`${area.title}  ${area.description}`);
		for (const report of reports.filter((r) => r.area === area.id)) console.log(`  ${report.id.padEnd(22)} ${report.title}: ${report.description}`);
	}
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

console.log(`\n  GitPrompt quality · ${selected.length} of ${reports.length} reports · ${fromRepo(PATHS.index)}\n`);
const ok = await run(reports, selected, { onStarted: () => flags.has('--no-open') || openPage(PATHS.index) });
process.exit(ok ? 0 : 1);

function openPage(path) {
	const [command, ...rest] = process.platform === 'darwin' ? ['open'] : process.platform === 'win32' ? ['cmd', '/c', 'start', '""'] : ['xdg-open'];
	spawn(command, [...rest, path], { stdio: 'ignore', detached: true }).unref();
}

function fail(message) {
	console.error(`quality: ${message}`);
	process.exit(2);
}
