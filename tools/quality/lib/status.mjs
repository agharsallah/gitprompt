// What the dashboard reads: status.js (every report's card and the build in progress, re-read
// every second while a build runs) and data/<area>/<name>.js (one report's page). Both are
// scripts, not JSON: a page opened from disk may load scripts but not fetch files.
import { writeFileSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

import { AREAS } from '../areas.mjs';
import { PATHS, fromHub } from './paths.mjs';

/** state.json: what the last run of each report left, kept across builds. */
export async function loadState() {
	const text = await readFile(PATHS.state, 'utf8').catch(() => null);
	const state = text ? JSON.parse(text) : {};
	return { reports: state.reports || {}, lastBuild: state.lastBuild || null };
}

export const dataPath = (report) => join(PATHS.data, report.area, `${report.name}.js`);

export async function writeData(report, page) {
	const path = dataPath(report);
	await mkdir(dirname(path), { recursive: true });
	await writeFile(path, `registerReport(${JSON.stringify(page)});\n`);
}

/** Writes state.json and status.js; sync, so it also works from a signal handler. */
export function writeStatus(reports, state, build) {
	writeFileSync(PATHS.state, JSON.stringify({ reports: state.reports, lastBuild: build }, null, 1));
	const status = {
		areas: AREAS,
		build,
		reports: reports.map((report) => ({
			id: report.id,
			area: report.area,
			title: report.title,
			description: report.description,
			command: `just quality ${report.id}`,
			log: fromHub(join(PATHS.logs, `${report.area}-${report.name}.log`)),
			data: fromHub(dataPath(report)),
			...state.reports[report.id]
		}))
	};
	writeFileSync(PATHS.status, `qualityStatus(${JSON.stringify(status)});\n`);
}
