// How the dashboard feels in use, measured in headless Chrome against the release daemon on demo
// data: first paint on a cold start and on a relaunch, switching pages from the sidebar, resizing
// the window, and what it costs while nobody touches it (main-thread CPU, requests, memory).
import { join } from 'node:path';

import { findChrome } from '../../lib/chrome.mjs';
import { HEIGHT, IDLE_S, ROUNDS, ROUTES, WIDTHS, loads, session } from '../../lib/dashboard-timings.mjs';
import { startDaemon } from '../../lib/process.mjs';
import { copyOf, drainedDemo, performanceEnv, releaseDaemonBinary, tempFolder, webBuild } from '../../lib/shared.mjs';
import { bars, kb, kpis, mb, ms, note, table, toneBelow } from '../../lib/spec.mjs';
import { repeat, summary } from '../../lib/stats.mjs';

const LOADS = 3;
const MB = 1048576;
const TONES = {
	paint: (v) => toneBelow(v, 500, 1000),
	ready: (v) => toneBelow(v, 1000, 2000),
	switch: (v) => toneBelow(v, 100, 300),
	frame: (v) => toneBelow(v, 50, 100),
	layout: (v) => toneBelow(v, 16, 50),
	longTasks: (v) => toneBelow(v, 50, 200),
	cpu: (v) => toneBelow(v, 0.5, 2),
	heapGrowth: (v) => toneBelow(v, 2 * MB, 10 * MB)
};

export default {
	title: 'Responsiveness',
	description: 'The dashboard in headless Chrome on the release daemon: cold and relaunch first paint, page switches, resizing, idle CPU and memory.',
	// The page probes it runs, shared with the desktop app's own performance run.
	inputs: ['backend/apps/desktop/src/perf/probes.js', '.env', '.env.performance'],
	needs: [
		{ command: 'pnpm', install: 'https://pnpm.io/installation' },
		{ command: 'cargo', install: 'https://rustup.rs' }
	],
	async run(ctx) {
		const chrome = findChrome();
		if (!chrome) throw new Error('no Chrome found: install Google Chrome, or set CHROME_PATH to a Chrome or Chromium binary');
		const [binary] = await Promise.all([releaseDaemonBinary(ctx), webBuild(ctx)]);
		const home = await copyOf(ctx, await drainedDemo(ctx));
		const profiles = await repeat(LOADS + 2, () => tempFolder(ctx));
		ctx.step('starting the release daemon on the demo data');
		const daemon = await startDaemon(binary, { home, dashboard: join(ctx.web, 'build'), env: performanceEnv() });
		ctx.cleanup(daemon.stop);
		// Timings on a machine busy with other reports would measure them, not the dashboard.
		const result = await ctx.alone('page timings', async () => {
			ctx.step('warming the daemon up (one untimed load)');
			await loads(chrome, daemon.url, profiles[LOADS + 1]);
			const runs = await repeat(LOADS, (i) => {
				ctx.step(`cold load and relaunch ${i + 1}/${LOADS}`);
				return loads(chrome, daemon.url, profiles[i]);
			});
			ctx.step(`page switches, resizing, ${IDLE_S} s idle`);
			return { runs, session: await session(chrome, daemon.url, profiles[LOADS]) };
		});
		return page(result);
	}
};

const cell = (value, toneOf, format = ms) => ({ text: value === null || value === undefined ? '–' : format(value), sort: value, tone: toneOf ? toneOf(value) : undefined });
const median = (values) => summary(values).median;

/** One row per load metric, the cold and relaunch medians side by side. */
function loadRows(runs) {
	const of = (kind, key) => median(runs.map((r) => r[kind][key]).filter((v) => v !== null));
	const rows = [
		['First contentful paint', 'fcp', TONES.paint, ms],
		['Largest contentful paint', 'lcp', TONES.paint, ms],
		['DOMContentLoaded', 'domContentLoaded', null, ms],
		['Load event', 'load', null, ms],
		['Data shown (settled)', 'ready', TONES.ready, ms],
		['Long tasks', 'longTaskMs', TONES.longTasks, ms],
		['Requests', 'requests', null, String],
		['Transferred', 'transferBytes', null, kb]
	];
	return rows.map(([metric, key, toneOf, format]) => ({ metric, cold: cell(of('cold', key), toneOf, format), relaunch: cell(of('relaunch', key), toneOf, format) }));
}

/** Per page: the first visit and the revisits' median and max. */
function switchRows(switches) {
	return ROUTES.map((route) => {
		const visits = switches.filter((s) => s.path === route.path);
		const first = visits.find((s) => s.firstVisit);
		const again = summary(visits.filter((s) => !s.firstVisit).map((s) => s.ms));
		return {
			page: `${route.name} (${route.path})`,
			first: cell(first?.ms ?? null, TONES.switch),
			again: cell(again.median, TONES.switch),
			max: cell(again.max, TONES.switch),
			longTasks: cell(Math.max(...visits.map((s) => s.longTaskMs)), TONES.longTasks),
			againMs: again.median
		};
	});
}

function page({ runs, session: s }) {
	const cold = (key) => median(runs.map((r) => r.cold[key]).filter((v) => v !== null));
	const relaunchReady = median(runs.map((r) => r.relaunch.ready));
	const switches = switchRows(s.switches);
	const firstVisits = median(s.switches.filter((v) => v.firstVisit).map((v) => v.ms));
	const revisits = median(s.switches.filter((v) => !v.firstVisit).map((v) => v.ms));
	const worstResize = s.resizes.reduce((worst, r) => (r.ms > worst.ms ? r : worst), s.resizes[0]);
	const heapGrowth = s.heapAfterSwitching - s.heapAtLoad;
	return {
		headline: { value: ms(cold('ready')), label: 'cold load until the data shows (median)', tone: TONES.ready(cold('ready')) },
		summary: `first paint cold ${ms(cold('fcp'))} · relaunch ${ms(relaunchReady)} · page switch ${ms(revisits)} · worst resize ${ms(worstResize.ms)} · idle ${s.idle.cpuPercent.toFixed(2)}% CPU`,
		sections: [
			kpis([
				{ label: 'First paint, cold', value: ms(cold('fcp')), tone: TONES.paint(cold('fcp')), hint: 'first contentful paint on an empty browser profile' },
				{ label: 'Data shown, cold', value: ms(cold('ready')), tone: TONES.ready(cold('ready')), hint: 'navigation start until the overview shows its data and painted' },
				{ label: 'Data shown, relaunch', value: ms(relaunchReady), tone: TONES.ready(relaunchReady), hint: 'the same, on the profile of the cold load (warm HTTP cache)' },
				{ label: 'Page switch', value: ms(revisits), tone: TONES.switch(revisits), hint: `sidebar click until the page settled, pages visited before (median of ${ROUNDS - 1} rounds)` },
				{ label: 'First visit', value: ms(firstVisits), tone: TONES.switch(firstVisits), hint: 'a page opened for the first time: its code and data load' },
				{ label: 'Worst resize', value: ms(worstResize.ms), tone: TONES.frame(worstResize.ms), hint: `${worstResize.path} from ${worstResize.from} to ${worstResize.to} px, until it painted again` },
				{ label: 'Idle main thread', value: `${s.idle.cpuPercent.toFixed(2)}%`, tone: TONES.cpu(s.idle.cpuPercent), hint: `busy share over ${IDLE_S} s untouched on the overview (${Math.round(s.idle.taskMs)} ms of tasks)` },
				{ label: 'Heap after switching', value: mb(s.heapAfterSwitching), tone: TONES.heapGrowth(heapGrowth), hint: `${heapGrowth >= 0 ? '+' : ''}${mb(heapGrowth)} since the first load, after garbage collection: what page switches leave behind` }
			]),
			table('Loads (median of cold and relaunch)', [{ key: 'metric', label: 'Metric' }, { key: 'cold', label: `Cold (${runs.length} empty profiles)`, align: 'right' }, { key: 'relaunch', label: 'Relaunch (same profile)', align: 'right' }], loadRows(runs), 'Each load in its own headless Chrome at 1280×840, times from navigation start. Relaunch reuses the HTTP cache of the cold load: what it still transfers is what the daemon does not let the browser keep.'),
			bars(
				'Page switch (pages visited before, median), against 300 ms',
				[...switches].sort((a, b) => b.againMs - a.againMs).map((row) => ({ label: row.page, value: Math.min(1, row.againMs / 300), text: ms(row.againMs), tone: TONES.switch(row.againMs) })),
				'Good under 100 ms: the switch feels instant.'
			),
			table('Page switches', [{ key: 'page', label: 'Page' }, { key: 'first', label: 'First visit', align: 'right' }, { key: 'again', label: 'Revisit (median)', align: 'right' }, { key: 'max', label: 'Revisit (max)', align: 'right' }, { key: 'longTasks', label: 'Worst long tasks', align: 'right' }], switches, `${ROUNDS} rounds through the sidebar. Long tasks: main-thread work over 50 ms during the switch, which delays input.`),
			table(
				`Resizing (height ${HEIGHT} px)`,
				[{ key: 'page', label: 'Page', mono: true }, { key: 'size', label: 'Width' }, { key: 'ms', label: 'Until painted', align: 'right' }, { key: 'layout', label: 'Layout + style', align: 'right' }, { key: 'script', label: 'Script', align: 'right' }],
				s.resizes.map((r) => ({ page: r.path, size: `${r.from} → ${r.to} px`, ms: cell(r.ms, TONES.frame), layout: cell(r.layoutMs, TONES.layout), script: cell(r.scriptMs) })),
				`Widths ${WIDTHS.join(', ')} px in turn. Until painted includes two DevTools round trips (about a millisecond each).`
			),
			table(
				`Idle (${IDLE_S} s untouched on the overview)`,
				[{ key: 'metric', label: 'Metric' }, { key: 'value', label: 'Value', align: 'right' }],
				[
					{ metric: 'Main thread busy', value: cell(s.idle.cpuPercent, TONES.cpu, (v) => `${v.toFixed(2)}%`) },
					{ metric: 'Requests sent', value: cell(s.idle.requests, (v) => toneBelow(v, 0, 3), String) },
					{ metric: 'JavaScript heap', value: cell(s.idle.heapBytes, null, mb) },
					{ metric: 'Heap growth while idle', value: cell(s.idle.heapGrowthBytes, TONES.heapGrowth, mb) },
					{ metric: 'DOM nodes', value: cell(s.idle.nodes, null, String) }
				],
				'The live stream (server-sent events) stays open and is not counted as a request.'
			),
			note('Headless Chrome, not the desktop app’s system webview (WebKit on macOS): the same page and daemon, another engine. Compare runs on the same machine; timings move a few milliseconds between runs.')
		]
	};
}
