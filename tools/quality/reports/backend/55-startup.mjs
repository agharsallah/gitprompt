// The daemon as the desktop app and `gitprompt serve` live with it: how long it takes to answer on a
// fresh install (empty data folder), on a first start with a spool to drain (hooks captured while
// it was not running: the demo prompts), and on a relaunch; its first reads, how long it takes to
// stop, and what it costs while nothing happens (CPU, memory). Release build, measured alone.
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { idleUsage, startDaemon, usage } from '../../lib/process.mjs';
import { demoHome, performanceEnv, releaseDaemonBinary, tempFolder, webBuild } from '../../lib/shared.mjs';
import { bars, count, kpis, mb, ms, note, table, toneBelow } from '../../lib/spec.mjs';
import { repeat, summary } from '../../lib/stats.mjs';

const RUNS = 3;
/** Seconds the daemon is left to finish its start-up work before idle is measured, and how long idle is measured. */
const SETTLE_S = 3;
const IDLE_S = 10;
/** The first reads a dashboard makes after the daemon answers. */
const READS = [
	{ name: 'Prompts list', path: '/api/prompts?limit=50' },
	{ name: 'Stats', path: '/api/stats' },
	{ name: 'Sources', path: '/api/sources' }
];
const TONES = {
	start: (v) => toneBelow(v, 250, 1000),
	drain: (v) => toneBelow(v, 2000, 10_000),
	read: (v) => toneBelow(v, 100, 300),
	stop: (v) => toneBelow(v, 500, 2000),
	cpu: (v) => toneBelow(v, 0.5, 2),
	rss: (v) => toneBelow(v, 60 * 1048576, 150 * 1048576)
};

export default {
	title: 'Startup',
	description: 'The release daemon: fresh install, first start draining a spool, relaunch, first reads, shutdown, idle CPU and memory.',
	inputs: ['.env', '.env.performance'],
	async run(ctx) {
		const [binary] = await Promise.all([releaseDaemonBinary(ctx), webBuild(ctx)]);
		const dashboard = join(ctx.web, 'build');
		const empties = await repeat(RUNS, () => tempFolder(ctx));
		const demo = await demoHome(ctx);
		const spooled = await spoolPayloads(demo);
		const env = performanceEnv();
		const start = (home) => startDaemon(binary, { home, dashboard, env, timeoutMs: 300_000 });
		// Start-up times on a machine busy with other reports would measure them, not the daemon.
		const result = await ctx.alone('start-ups', async () => {
			const fresh = await repeat(RUNS, (i) => {
				ctx.step(`fresh install ${i + 1}/${RUNS}`);
				return startAndStop(start, empties[i]);
			});
			ctx.step(`first start draining ${count(spooled)} spooled payloads`);
			const drain = await startAndStop(start, demo);
			const relaunches = await repeat(RUNS, (i) => {
				ctx.step(`relaunch ${i + 1}/${RUNS}${i === RUNS - 1 ? `, then ${SETTLE_S + IDLE_S} s idle` : ''}`);
				return startAndStop(start, demo, { idle: i === RUNS - 1 });
			});
			return { fresh, drain, relaunches };
		});
		return page({ ...result, spooled });
	}
};

/** Lines in the spool files of a data folder: the payloads a start drains before it answers. */
async function spoolPayloads(home) {
	const folder = join(home, 'spool');
	const files = await readdir(folder).catch(() => []);
	const texts = await Promise.all(files.map((file) => readFile(join(folder, file), 'utf8')));
	return texts.reduce((total, text) => total + text.split('\n').filter(Boolean).length, 0);
}

/** One start on `home`: { startMs, reads, stopMs, rssBytes, idle }; idle only when asked. */
async function startAndStop(start, home, { idle = false } = {}) {
	const daemon = await start(home);
	let stopMs = null;
	try {
		const reads = await firstReads(daemon.url);
		const rss = (await usage(daemon.pid))?.rssBytes ?? null;
		const quiet = idle ? await idleAfterSettling(daemon.pid) : null;
		const stopping = performance.now();
		await daemon.stop();
		stopMs = performance.now() - stopping;
		return { startMs: daemon.startMs, reads, stopMs, rssBytes: rss, idle: quiet };
	} finally {
		if (stopMs === null) await daemon.stop();
	}
}

/** Each first read, one after another: { [path]: ms }. */
async function firstReads(url) {
	const times = {};
	for (const read of READS) {
		const begun = performance.now();
		const response = await fetch(`${url}${read.path}`);
		await response.arrayBuffer();
		if (!response.ok) throw new Error(`${read.path} answered ${response.status}`);
		times[read.path] = performance.now() - begun;
	}
	return times;
}

async function idleAfterSettling(pid) {
	await new Promise((resolve) => setTimeout(resolve, SETTLE_S * 1000));
	return idleUsage(pid, IDLE_S);
}

const timing = (value, toneOf) => ({ text: value === null ? '–' : ms(value), sort: value, tone: toneOf(value) });
const bar = (label, value, toneOf, scale) => ({ label, value: Math.min(1, value / scale), text: ms(value), tone: toneOf(value) });

function page({ fresh, drain, relaunches, spooled }) {
	const freshStart = summary(fresh.map((r) => r.startMs));
	const relaunch = summary(relaunches.map((r) => r.startMs));
	const stops = summary([...fresh, drain, ...relaunches].map((r) => r.stopMs));
	const { idle } = relaunches.at(-1);
	const rss = relaunches.at(-1).rssBytes;
	const perPayload = spooled ? drain.startMs / spooled : null;
	const readMedian = (path) => summary(relaunches.map((r) => r.reads[path])).median;
	return {
		headline: { value: ms(relaunch.median), label: 'relaunch, until it answers (median)', tone: TONES.start(relaunch.median) },
		summary: `fresh ${ms(freshStart.median)} · relaunch ${ms(relaunch.median)} · ${count(spooled)} spooled payloads ${ms(drain.startMs)} · stop ${ms(stops.median)}${idle ? ` · idle ${idle.cpuPercent.toFixed(2)}% CPU, ${mb(idle.rssBytes)}` : ''}`,
		sections: [
			kpis([
				{ label: 'Fresh install', value: ms(freshStart.median), tone: TONES.start(freshStart.median), hint: `spawn until /api/status answers, on an empty data folder; max ${ms(freshStart.max)}` },
				{ label: 'Relaunch', value: ms(relaunch.median), tone: TONES.start(relaunch.median), hint: `the demo data folder again, database built; max ${ms(relaunch.max)}` },
				{ label: 'First start with a spool', value: ms(drain.startMs), tone: TONES.drain(drain.startMs), hint: `${count(spooled)} payloads spooled while no daemon ran, drained before it answers${perPayload ? ` (${perPayload.toFixed(1)} ms each)` : ''}` },
				{ label: 'Shutdown', value: ms(stops.median), tone: TONES.stop(stops.median), hint: `Ctrl-C to exit, over every start; max ${ms(stops.max)}` },
				{ label: 'Idle CPU', value: idle ? `${idle.cpuPercent.toFixed(2)}%` : '–', tone: idle ? TONES.cpu(idle.cpuPercent) : 'neutral', hint: idle ? `of one core over ${IDLE_S} s with nothing captured (${Math.round(idle.cpuMs)} ms of CPU)` : 'no reading on this platform' },
				{ label: 'Memory at start', value: rss === null ? '–' : mb(rss), tone: TONES.rss(rss), hint: 'resident after a relaunch and its first reads' },
				{ label: 'Memory idle', value: idle ? mb(idle.rssBytes) : '–', tone: idle ? TONES.rss(idle.rssBytes) : 'neutral', hint: `resident after ${SETTLE_S + IDLE_S} s idle` }
			]),
			bars(
				'Until the daemon answers, against 1 s',
				[bar('Fresh install', freshStart.median, TONES.start, 1000), bar('Relaunch', relaunch.median, TONES.start, 1000), bar(`Draining ${count(spooled)} spooled payloads`, drain.startMs, TONES.drain, 1000), bar('Shutdown', stops.median, TONES.stop, 1000)],
				`Medians of ${RUNS} runs (one for the spool). A full bar is one second or more. The dashboard shows nothing until the daemon answers.`
			),
			table(
				'First reads after a relaunch (median)',
				[
					{ key: 'name', label: 'Read' },
					{ key: 'path', label: 'Path', mono: true },
					{ key: 'relaunch', label: 'After relaunch', align: 'right' },
					{ key: 'drain', label: 'After the spool drain', align: 'right' }
				],
				READS.map((read) => ({ name: read.name, path: read.path, relaunch: timing(readMedian(read.path), TONES.read), drain: timing(drain.reads[read.path], TONES.read) })),
				'The first answers the dashboard waits for once the daemon is up, caches still empty.'
			),
			table(
				'Starts',
				[
					{ key: 'kind', label: 'Start' },
					{ key: 'start', label: 'Until it answers', align: 'right' },
					{ key: 'stop', label: 'Shutdown', align: 'right' },
					{ key: 'rss', label: 'Memory', align: 'right' }
				],
				[...fresh.map((r, i) => ({ kind: `Fresh install ${i + 1}`, ...r })), { kind: 'Spool drain', ...drain }, ...relaunches.map((r, i) => ({ kind: `Relaunch ${i + 1}`, ...r }))].map((r) => ({
					kind: r.kind,
					start: timing(r.startMs, r.kind === 'Spool drain' ? TONES.drain : TONES.start),
					stop: timing(r.stopMs, TONES.stop),
					rss: { text: r.rssBytes === null ? '–' : mb(r.rssBytes), sort: r.rssBytes }
				}))
			),
			note('Release build of `gitprompt serve` in the performance profile (`.env.performance`), alone on the machine; the desktop app starts the same daemon in-process. Starts are polled every 10 ms, so they read up to 10 ms late. The spool is the demo data of tools/seed-demo.py.')
		]
	};
}
