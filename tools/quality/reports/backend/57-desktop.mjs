// The real desktop app (release build, system webview: WebKit on macOS) in its performance profile
// (.env.performance, backend/apps/desktop/src/perf): launch milestones until the dashboard shows
// its data, cold (private webview store) and relaunched (warm cache), page switches, real window
// resizes, and what the app and its webview processes cost while idle. It opens a window and takes
// the screen for a minute or two, so it runs only when named: `just quality desktop`.
import { join } from 'node:path';

import { launchApp } from '../../lib/desktop-launch.mjs';
import { freePort } from '../../lib/process.mjs';
import { copyOf, desktopBinary, drainedDemo, performanceEnv, webBuild } from '../../lib/shared.mjs';
import { bars, kpis, mb, metric, ms, note, table, toneBelow } from '../../lib/spec.mjs';
import { repeat, summary } from '../../lib/stats.mjs';

const TONES = {
	launch: (v) => toneBelow(v, 1000, 2500),
	window: (v) => toneBelow(v, 500, 1500),
	switch: (v) => toneBelow(v, 100, 300),
	resize: (v) => toneBelow(v, 50, 100),
	cpu: (v) => toneBelow(v, 1, 5),
	memory: (v) => toneBelow(v, 200 * 1048576, 400 * 1048576)
};
/** Launch milestones, each an epoch time from one run (marks of the app, timings of the page). */
const MILESTONES = [
	{ label: 'Process running (main)', at: (r) => r.marks.main },
	{ label: 'Tauri set up', at: (r) => r.marks.setup },
	{ label: 'Daemon ready', at: (r) => r.marks.daemonReady },
	{ label: 'Window built', at: (r) => r.marks.window },
	{ label: 'Page loaded', at: (r) => r.marks.pageLoaded },
	{ label: 'First paint', at: (r) => (r.load.fcp === null ? null : r.load.timeOrigin + r.load.fcp) },
	{ label: 'Data shown', at: (r) => r.load.timeOrigin + r.load.ready }
];
const DATA_SHOWN = MILESTONES.length - 1;

export default {
	title: 'Desktop app',
	description: 'The real desktop app and its webview: launch to data shown, cold and relaunched, page switches, window resizes, idle CPU and memory. Opens a window: only when named (`just quality desktop`).',
	/** A measurement: runs alone, with `just quality measure` or when named, not with every build. */
	measure: true,
	optIn: true,
	inputs: ['web/src', '.env', '.env.performance'],
	async run(ctx) {
		const env = performanceEnv();
		const launches = Number(env.GITPROMPT_PERF_LAUNCHES) || 3;
		const idleSeconds = Number(env.GITPROMPT_PERF_IDLE_SECONDS) || 15;
		const [binary] = await Promise.all([desktopBinary(ctx), webBuild(ctx)]);
		const drained = await drainedDemo(ctx);
		const homes = await repeat(launches + 1, () => copyOf(ctx, drained));
		// One port for every launch: the webview caches by address, so a relaunch finds what the
		// launch before it cached.
		const port = await freePort();
		const launch = (step, slug, options) => {
			ctx.step(step);
			return launchApp(binary, { env, port, dashboard: join(ctx.web, 'build'), out: join(ctx.artifacts, `${slug}.json`), log: (text) => ctx.log(text.trimEnd()), ...options });
		};
		// Launch times on a machine busy with other reports would measure them, not the app.
		const runs = await ctx.alone('desktop launches', async () => {
			await launch('warming the webview cache (one untimed launch)', 'warm-up', { home: homes[launches], cold: false });
			return repeat(launches, async (i) => ({
				cold: await launch(`cold launch ${i + 1}/${launches}`, `cold-${i + 1}`, { home: homes[i], cold: true }),
				relaunch: await launch(`relaunch ${i + 1}/${launches}${i === launches - 1 ? `, then ${idleSeconds} s idle` : ''}`, `relaunch-${i + 1}`, {
					home: homes[i],
					cold: false,
					idleSeconds: i === launches - 1 ? idleSeconds : 0
				})
			}));
		});
		return page(runs, idleSeconds);
	}
};

/** Median ms since spawn at which each milestone came, over the launches given. */
function timeline(launches) {
	const since = (m, launch) => {
		const at = m.at(launch.report);
		return at === null || at === undefined ? null : at - launch.spawnedAt;
	};
	return MILESTONES.map((m) => summary(launches.map((l) => since(m, l)).filter((v) => v !== null)).median);
}

const cell = (value, toneOf) => ({ text: value === null || value === undefined ? '–' : ms(value), sort: value, tone: toneOf?.(value) });

function switchRows(visits) {
	const paths = [...new Set(visits.map((v) => v.path))];
	return paths.map((path) => {
		const mine = visits.filter((v) => v.path === path);
		const again = summary(mine.filter((v) => !v.firstVisit).map((v) => v.ms));
		const first = summary(mine.filter((v) => v.firstVisit).map((v) => v.ms));
		return { path, first: cell(first.median, TONES.switch), again: cell(again.median, TONES.switch), max: cell(again.max, TONES.switch), againMs: again.median };
	});
}

function page(runs, idleSeconds) {
	const cold = timeline(runs.map((r) => r.cold));
	const warm = timeline(runs.map((r) => r.relaunch));
	// Switches and resizes of the relaunches: the app as it usually runs, cache warm.
	const visits = runs.flatMap((r) => r.relaunch.report.switches);
	const resizes = runs.flatMap((r) => r.relaunch.report.resizes);
	const revisit = summary(visits.filter((v) => !v.firstVisit).map((v) => v.ms)).median;
	const firstVisit = summary(visits.filter((v) => v.firstVisit).map((v) => v.ms)).median;
	const worstResize = resizes.reduce((worst, r) => (r.ms > worst.ms ? r : worst), resizes[0]);
	const { idle } = runs.at(-1).relaunch;
	const nodes = runs.at(-1).relaunch.report.inspect.nodes;
	const windowAt = MILESTONES.findIndex((m) => m.label === 'Window built');
	const switches = switchRows(visits);
	return {
		headline: { value: ms(cold[DATA_SHOWN]), label: 'cold launch until the data shows (median)', tone: TONES.launch(cold[DATA_SHOWN]) },
		trend: [metric('cold', 'Cold launch until the data shows', cold[DATA_SHOWN], 'ms', 'lower'), metric('relaunch', 'Relaunch until the data shows', warm[DATA_SHOWN], 'ms', 'lower'), metric('switch', 'Page switch', revisit, 'ms', 'lower')],
		summary: `cold ${ms(cold[DATA_SHOWN])} · relaunch ${ms(warm[DATA_SHOWN])} · page switch ${ms(revisit)} · worst resize ${ms(worstResize.ms)}${idle ? ` · idle ${idle.cpuPercent.toFixed(1)}% CPU, ${mb(idle.rssBytes)}` : ''}`,
		sections: [
			kpis([
				{ label: 'Launch to data, cold', value: ms(cold[DATA_SHOWN]), tone: TONES.launch(cold[DATA_SHOWN]), hint: 'spawn until the overview shows its data, empty webview store' },
				{ label: 'Relaunch to data', value: ms(warm[DATA_SHOWN]), tone: TONES.launch(warm[DATA_SHOWN]), hint: 'the same with the webview cache of earlier launches' },
				{ label: 'Window, cold', value: ms(cold[windowAt]), tone: TONES.window(cold[windowAt]), hint: 'spawn until the window is built (after the daemon is ready)' },
				{ label: 'Page switch', value: ms(revisit), tone: TONES.switch(revisit), hint: 'sidebar click until the page settled, pages visited before' },
				{ label: 'First visit', value: ms(firstVisit), tone: TONES.switch(firstVisit), hint: 'a page opened for the first time in a launch' },
				{ label: 'Worst resize', value: ms(worstResize.ms), tone: TONES.resize(worstResize.ms), hint: `${worstResize.path} from ${worstResize.from} to ${worstResize.to} px: window asked to resize until the page painted` },
				{ label: 'Idle CPU', value: idle ? `${idle.cpuPercent.toFixed(1)}%` : '–', tone: idle ? TONES.cpu(idle.cpuPercent) : 'neutral', hint: idle ? `the app and its webview processes, of one core, untouched on the overview` : 'no reading on this platform' },
				{ label: 'Memory', value: idle ? mb(idle.rssBytes) : '–', tone: idle ? TONES.memory(idle.rssBytes) : 'neutral', hint: 'resident, the app and its webview processes together (shared pages counted in each)' },
				{ label: 'DOM nodes', value: nodes ?? '–', hint: 'elements on the overview after the run' }
			]),
			table(
				'Launch timeline (ms since spawn, median)',
				[{ key: 'step', label: 'Milestone' }, { key: 'cold', label: `Cold (${runs.length})`, align: 'right' }, { key: 'warm', label: `Relaunch (${runs.length})`, align: 'right' }],
				MILESTONES.map((m, i) => ({ step: m.label, cold: cell(cold[i]), warm: cell(warm[i]) })),
				'Cold: a private webview store (nothing cached) on a fresh copy of the data. Relaunch: the same data again with the cache of the launches before. Files the system already cached stay cached: a launch after a reboot is slower.'
			),
			bars(
				'Page switch (pages visited before, median), against 300 ms',
				[...switches].sort((a, b) => b.againMs - a.againMs).map((s) => ({ label: s.path, value: Math.min(1, s.againMs / 300), text: ms(s.againMs), tone: TONES.switch(s.againMs) }))
			),
			table('Page switches (relaunches)', [{ key: 'path', label: 'Page', mono: true }, { key: 'first', label: 'First visit', align: 'right' }, { key: 'again', label: 'Revisit (median)', align: 'right' }, { key: 'max', label: 'Revisit (max)', align: 'right' }], switches),
			table(
				'Window resizes (relaunches)',
				[{ key: 'page', label: 'Page', mono: true }, { key: 'size', label: 'Width' }, { key: 'ms', label: 'Until painted', align: 'right' }],
				resizes.map((r) => ({ page: r.path, size: `${r.from} → ${r.to} px`, ms: cell(r.ms, TONES.resize) })),
				'The native window resized from Rust; timed from the call until the page painted at its new width (both on the system clock).'
			),
			...(idle
				? [
						table(
							`Idle processes (${idleSeconds} s untouched)`,
							[{ key: 'name', label: 'Process', mono: true }, { key: 'cpu', label: 'CPU', align: 'right' }, { key: 'rss', label: 'Memory', align: 'right' }],
							idle.processes.map((p) => ({ name: p.name, cpu: { text: `${p.cpuPercent.toFixed(2)}%`, sort: p.cpuPercent }, rss: { text: mb(p.rssBytes), sort: p.rssBytes } })),
							'The app and the webview helpers it started (on macOS: the WebKit processes that appeared with it).'
						)
					]
				: []),
			note('Release build of `gitprompt-desktop` in the performance profile (`.env.performance`) on the drained demo data, alone on the machine. The window must stay visible: a hidden window stops painting.')
		]
	};
}
