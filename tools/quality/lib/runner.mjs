// Runs the selected reports side by side (scheduler.mjs). Every change (report started, step,
// output line, finished) reaches status.js within a moment, so an open dashboard shows the build
// as it goes; each finished report's page lands in data/. A failed or skipped report never stops
// the others. A report whose inputs did not change since its last success keeps its result
// (fingerprint.mjs), unless `force`.
import { cp, mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { createContext, missing, stopChildren } from './context.mjs';
import { fingerprints, headCommit, upToDate } from './fingerprint.mjs';
import { addRun, loadHistory, writeHistory } from './history.mjs';
import { PATHS, OUT, REPO, fromRepo } from './paths.mjs';
import { createProgress, duration } from './progress.mjs';
import { QUICK_JOBS, gate, schedule } from './scheduler.mjs';
import { dataPath, loadState, writeData, writeStatus } from './status.mjs';

/** Output lines kept per running report for the dashboard. */
const TAIL_LINES = 14;
/** status.js is rewritten at most this often while output streams in. */
const SAVE_EVERY_MS = 300;
/**
 * status.js is rewritten at least this often while a build runs (build.heartbeat_at), so the page
 * tells a build that is quiet from one that was killed (hub/build.js: STALE_MS).
 */
const HEARTBEAT_MS = 3_000;
/** What stops a build from the outside: Ctrl-C, `kill`, a closed terminal. */
const SIGNALS = ['SIGINT', 'SIGTERM', 'SIGHUP'];

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

/** Resolves true when every selected report succeeded (skipped and up-to-date ones count as fine). */
export async function run(all, selected, { jobs, force = false, onStarted } = {}) {
	await installHub();
	const state = await loadState();
	const expected = (report) => state.reports[report.id]?.work_ms ?? state.reports[report.id]?.duration_ms;
	// Every report's inputs, not only the selected ones': the dashboard says which results left out
	// of this build (measurements, say) are older than the code.
	const [hashes, commit] = await Promise.all([fingerprints(REPO, all, (report) => report.inputs), headCommit(REPO)]);
	const history = loadHistory(PATHS.history);
	writeHistory(history, { json: PATHS.history, script: PATHS.historyScript });
	const kept = force ? new Set() : await upToDate(selected, state.reports, hashes, hasPage);
	const changed = all.filter((report) => !selected.includes(report) && state.reports[report.id]?.inputs && state.reports[report.id].inputs !== hashes[report.id]);
	const build = {
		started_at: Date.now(),
		running: true,
		jobs,
		quick_jobs: QUICK_JOBS,
		changed: changed.map((report) => report.id),
		steps: selected.map((report) => (kept.has(report.id) ? { id: report.id, state: 'fresh', built_at: state.reports[report.id].generated_at } : { id: report.id, state: 'queued', expected_ms: expected(report) }))
	};
	const stepOf = (report) => build.steps.find((step) => step.id === report.id);
	// Measurements wait until nothing else is left: no report running except those waiting to
	// measure themselves, none queued unless it waits for one of those.
	const reportOf = (step) => selected.find((report) => report.id === step.id);
	const idle = () => {
		const measuring = new Set(build.steps.filter((s) => s.measuring).map((s) => s.id));
		const afterMeasurement = (step) => (reportOf(step).after || []).some((id) => measuring.has(id));
		return !build.steps.some((s) => (s.state === 'queued' && !afterMeasurement(s)) || (s.state === 'running' && !s.measuring));
	};
	const shared = { shared: new Map(), owners: new Map(), cleanups: [], gate: gate(idle) };
	const save = throttled(() => {
		build.heartbeat_at = Date.now();
		writeStatus(all, state, build);
	}, SAVE_EVERY_MS);
	save.now();
	const heartbeat = setInterval(save, HEARTBEAT_MS);
	onStarted?.();
	const progress = createProgress(build, Math.max(...selected.map((report) => report.id.length)));

	const interrupted = () => {
		clearInterval(heartbeat);
		stopChildren();
		progress.stop();
		for (const step of build.steps.filter((s) => s.state === 'running' || s.state === 'queued')) {
			step.state = step.state === 'running' ? 'failed' : 'skipped';
			step.message = 'interrupted';
		}
		finish(build);
		save.now();
		for (const cleanup of shared.cleanups) Promise.resolve().then(cleanup).catch(() => {});
		process.exit(130);
	};
	for (const signal of SIGNALS) process.once(signal, interrupted);
	for (const step of build.steps.filter((s) => s.state === 'fresh')) progress.kept(step, state.reports[step.id]);

	await schedule(selected.filter((report) => !kept.has(report.id)), {
		jobs,
		expected,
		onWaiting: (report, ids) => {
			stepOf(report).waiting = `after ${ids.join(', ')}`;
		},
		runReport: async (report, slots) => {
			const step = stepOf(report);
			Object.assign(step, { state: 'running', started_at: Date.now(), detail: 'starting', tail: [] });
			delete step.waiting;
			save.now();
			try {
				const result = await runOne(report, { build: shared, slots, live: liveView(step, save, shared.gate) });
				Object.assign(step, { state: result.state, finished_at: Date.now(), message: result.message });
				for (const key of ['detail', 'activity', 'tail', 'waiting', 'measuring']) delete step[key];
				// The inputs hashed when the build started: a file edited meanwhile makes the next build run it again.
				const inputs = result.state === 'done' ? { inputs: hashes[report.id] } : {};
				state.reports[report.id] = { ...state.reports[report.id], ...result.record, ...inputs };
				if (result.state === 'done' && addRun(history, report.id, { at: result.record.generated_at, commit, trend: result.trend })) {
					writeHistory(history, { json: PATHS.history, script: PATHS.historyScript });
				}
				save.now();
				progress.finished(step, result);
			} finally {
				slots.release();
				shared.gate.poke();
			}
		}
	});

	for (const cleanup of shared.cleanups.reverse()) await Promise.resolve().then(cleanup).catch(() => {});
	for (const signal of SIGNALS) process.removeListener(signal, interrupted);
	clearInterval(heartbeat);
	progress.stop();
	finish(build);
	save.now();
	console.log(`\n  ${summary(build)}`);
	return build.steps.every((s) => s.state !== 'failed');
}

/** What a report's ctx tells the build: its step, its latest output, what it waits for. */
function liveView(step, save, lock) {
	return {
		measuring(flag) {
			step.measuring = flag;
			lock.poke();
		},
		step(text) {
			step.detail = text;
			save();
		},
		activity(text) {
			step.activity = text.slice(0, 240);
			save();
		},
		line(text) {
			step.tail.push(text.slice(0, 400));
			if (step.tail.length > TAIL_LINES) step.tail.shift();
			save();
		},
		waiting(text) {
			if (text) step.waiting = text;
			else delete step.waiting;
			save();
		}
	};
}

function finish(build) {
	build.running = false;
	build.finished_at = Date.now();
}

/** `9 of 10 reports built in 5m12s (12m40s of work, 4 at a time), 1 failed; 1 up to date …` */
function summary(build) {
	const fresh = build.steps.filter((s) => s.state === 'fresh').length;
	const total = build.steps.length - fresh;
	if (total === 0) return 'every report is up to date: no input changed since its last build (--force rebuilds them)';
	const kept = fresh ? `; ${fresh} up to date (inputs unchanged; --force rebuilds them)` : '';
	const failed = build.steps.filter((s) => s.state === 'failed').length;
	const work = build.steps.reduce((sum, s) => sum + (s.finished_at && s.started_at ? s.finished_at - s.started_at : 0), 0);
	const parallel = build.jobs > 1 && total > 1 ? ` (${duration(work)} of work, ${build.jobs} at a time)` : '';
	return `${total - failed} of ${total} reports built in ${duration(build.finished_at - build.started_at)}${parallel}${failed ? `, ${failed} failed` : ''}${kept}`;
}

const hasPage = (report) =>
	stat(dataPath(report)).then(
		() => true,
		() => false
	);

/** fn() at most every `ms` (the last call always lands); fn.now() at once. */
function throttled(fn, ms) {
	let timer = null;
	const call = () => {
		if (!timer) timer = setTimeout(call.now, ms);
	};
	call.now = () => {
		clearTimeout(timer);
		timer = null;
		fn();
	};
	return call;
}

/** One report: { state, line (terminal summary), message?, log, record (kept in state.json), trend? (history.json) }. */
async function runOne(report, options) {
	const started = Date.now();
	const absent = missing(report.needs);
	if (absent.length > 0) {
		const message = `needs ${absent.map((need) => `${need.command} (${need.install})`).join(', ')}`;
		return { state: 'skipped', line: message, message, record: { state: 'skipped', message, checked_at: started } };
	}
	const ctx = await createContext(report, options);
	try {
		const page = await report.run(ctx);
		const generated_at = Date.now();
		const duration_ms = generated_at - started;
		const work_ms = duration_ms - ctx.waitedMs;
		await writeData(report, { ...page, id: report.id, version: generated_at, generated_at, duration_ms });
		const record = { state: 'done', version: generated_at, generated_at, duration_ms, work_ms, headline: page.headline, summary: page.summary, message: null };
		return { state: 'done', line: `${page.headline.value} ${page.headline.label}`, log: ctx.logPath, record, trend: page.trend };
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		ctx.log(`\n# failed: ${message}`);
		return { state: 'failed', line: message.split('\n')[0], message, log: fromRepo(ctx.logPath), record: { state: 'failed', message, failed_at: Date.now(), duration_ms: Date.now() - started } };
	} finally {
		await ctx.close();
	}
}
