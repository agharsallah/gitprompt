// Finds the reports: every reports/<area>/NN-<name>.mjs, in area then file-name order. Its id is
// `<area>/<name>`; NN only orders the sidebar. Its inputs (what its result depends on, see
// fingerprint.mjs): the area's, its own `inputs`, its file, and the dashboard code it runs with.
import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

import { AREAS } from '../areas.mjs';
import { PATHS, fromRepo } from './paths.mjs';

/** What every report also depends on: the dashboard's own code (not the page) and the recipes it runs. */
const TOOL_INPUTS = ['tools/quality/lib', 'tools/quality/areas.mjs', 'justfile'];

export async function discover() {
	const reports = [];
	for (const area of AREAS) {
		const dir = join(PATHS.reports, area.id);
		const files = (await readdir(dir).catch(() => [])).filter((file) => file.endsWith('.mjs')).sort();
		for (const file of files) {
			const module = (await import(pathToFileURL(join(dir, file)).href)).default;
			const name = file.replace(/^\d+-/, '').replace(/\.mjs$/, '');
			if (!module?.title || typeof module.run !== 'function') {
				throw new Error(`reports/${area.id}/${file} must export default { title, description, run(ctx) }`);
			}
			const inputs = [...new Set([...TOOL_INPUTS, ...(area.inputs || []), ...(module.inputs || []), fromRepo(join(dir, file))])];
			reports.push({ needs: [], description: '', ...module, id: `${area.id}/${name}`, area: area.id, name, inputs });
		}
	}
	return reports;
}

/**
 * The reports matching the selectors: an area (`web`), an id (`backend/lint`) or a name in any area
 * (`lint`). No selector: every report. Throws on a selector that matches nothing.
 */
export function select(reports, selectors) {
	if (selectors.length === 0) return reports;
	const unknown = selectors.filter((s) => !reports.some((r) => matches(r, s)));
	if (unknown.length > 0) {
		throw new Error(`no report matches ${unknown.join(', ')}; there are: ${reports.map((r) => r.id).join(', ')}`);
	}
	return reports.filter((r) => selectors.some((s) => matches(r, s)));
}

function matches(report, selector) {
	return selector === report.id || selector === report.area || selector === report.name;
}
