// Runs the selected reports one after another. Every change (report started, step, finished) is
// written to status.js at once, so an open dashboard shows the build as it goes; each finished
// report's page lands in data/. A failed or skipped report never stops the others.
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { createContext, missing } from './context.mjs';
import { PATHS, OUT, fromRepo } from './paths.mjs';
import { createProgress, duration } from './progress.mjs';
import { loadState, writeData, writeStatus } from './status.mjs';

/**
 * Copies the dashboard shell (hub/) next to the data. Its own scripts and stylesheet get a new URL
 * each time, so a browser never runs an older copy of the page from its cache.
 */
export async function installHub() {
	await mkdir(OUT, { recursive: true });
	await cp(PATHS.hub, OUT, { recursive: true });
	const page = await readFile(join(PATHS.hub, 'index.html'), 'utf8');
	await writeFile(PATHS.index, page.replace(/(src|href)="([\w-]+\.(?:js|css))"/g, `$1="$2?v=${Date.now()}"`));
}

/** Resolves true when every selected report succeeded (skipped ones count as fine). */
export async function run(all, selected, { onStarted } = {}) {
	await installHub();
	const state = await loadState();
	const build = {
		started_at: Date.now(),
		running: true,
		steps: selected.map((report) => ({ id: report.id, state: 'queued' }))
	};
	const shared = { shared: new Map(), cleanups: [] };
	const save = () => writeStatus(all, state, build);
	save();
	onStarted?.();

	const interrupted = () => {
		for (const step of build.steps.filter((s) => s.state === 'running' || s.state === 'queued')) {
			step.state = step.state === 'running' ? 'failed' : 'skipped';
			step.message = 'interrupted';
		}
		finish(build);
		save();
		for (const cleanup of shared.cleanups) Promise.resolve().then(cleanup).catch(() => {});
		process.exit(130);
	};
	process.once('SIGINT', interrupted);

	const width = Math.max(...selected.map((report) => report.id.length));
	const progress = createProgress(selected.length, width);
	for (const [index, report] of selected.entries()) {
		const step = build.steps[index];
		Object.assign(step, { state: 'running', started_at: Date.now(), detail: 'starting' });
		save();
		progress.start(index, report);
		const result = await runOne(report, shared, (text) => {
			step.detail = text;
			progress.step(text);
			save();
		});
		Object.assign(step, { state: result.state, finished_at: Date.now(), message: result.message });
		delete step.detail;
		state.reports[report.id] = { ...state.reports[report.id], ...result.record };
		save();
		progress.end(result.state, result.line);
		if (result.state === 'failed') progress.detail(`${result.message}\n(full log: ${fromRepo(result.log)})`);
	}

	for (const cleanup of shared.cleanups.reverse()) await Promise.resolve().then(cleanup).catch(() => {});
	process.removeListener('SIGINT', interrupted);
	finish(build);
	save();
	const failed = build.steps.filter((s) => s.state === 'failed').length;
	console.log(`\n  ${selected.length - failed} of ${selected.length} reports built in ${duration(build.finished_at - build.started_at)}${failed ? `, ${failed} failed` : ''}`);
	return failed === 0;
}

function finish(build) {
	build.running = false;
	build.finished_at = Date.now();
}

/** One report: { state, line (terminal summary), message?, log, record (kept in state.json) }. */
async function runOne(report, shared, onStep) {
	const started = Date.now();
	const absent = missing(report.needs);
	if (absent.length > 0) {
		const message = `needs ${absent.map((need) => `${need.command} (${need.install})`).join(', ')}`;
		return { state: 'skipped', line: message, message, record: { state: 'skipped', message, checked_at: started } };
	}
	const ctx = await createContext(report, shared, onStep);
	try {
		const page = await report.run(ctx);
		const generated_at = Date.now();
		const duration_ms = generated_at - started;
		await writeData(report, { ...page, id: report.id, version: generated_at, generated_at, duration_ms });
		const record = { state: 'done', version: generated_at, generated_at, duration_ms, headline: page.headline, summary: page.summary, message: null };
		return { state: 'done', line: `${page.headline.value} ${page.headline.label}`, log: ctx.logPath, record };
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		ctx.log(`\n# failed: ${message}`);
		return { state: 'failed', line: message.split('\n')[0], message, log: ctx.logPath, record: { state: 'failed', message, failed_at: Date.now(), duration_ms: Date.now() - started } };
	} finally {
		await ctx.close();
	}
}
