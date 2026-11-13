// The dashboard timed in headless Chrome, against a daemon at `url` (the web report's measurements):
//   loads(chrome, url, profile)    a cold load on an empty profile, then a relaunch on the same one
//                                  (warm HTTP cache), each in its own browser.
//   session(chrome, url, profile)  one browser kept open: page switches through the sidebar, window
//                                  resizes, then a quiet spell (main-thread CPU, requests, memory).
import { launchChrome } from './chrome.mjs';
import { PROBES, call } from './page-probes.mjs';
import { repeat } from './stats.mjs';

/** The pages switched between, in sidebar order; the session starts and ends on the first. */
export const ROUTES = [
	{ path: '/', name: 'Overview' },
	{ path: '/prompts', name: 'Prompts' },
	{ path: '/sessions', name: 'Sessions' },
	{ path: '/projects', name: 'Projects' },
	{ path: '/sources', name: 'Connectors' }
];
/** Widths the window is resized through, from the 1280 px it opens at (the desktop window's size). */
export const WIDTHS = [1024, 768, 600, 1280];
export const HEIGHT = 840;
/** The pages resized: the charts of the overview and the longest list. */
export const RESIZED = ['/', '/prompts'];
export const ROUNDS = 3;
export const IDLE_S = 15;

/** A browser on `profile` with the dashboard open and settled: fn(page) runs, then the browser closes. */
async function withPage(chrome, url, profile, fn) {
	const browser = await launchChrome(chrome, { profile, height: HEIGHT });
	try {
		return await fn(await browser.open(`${url}/`, { preload: PROBES }));
	} finally {
		await browser.close();
	}
}

/** { cold, relaunch }: firstLoad() on an empty profile, then again on the same profile. */
export async function loads(chrome, url, profile) {
	const firstLoad = () => withPage(chrome, url, profile, (page) => page.evaluate(call('firstLoad')));
	const cold = await firstLoad();
	const relaunch = await firstLoad();
	return { cold, relaunch };
}

/** { heapAtLoad, switches, heapAfterSwitching, resizes, idle } of one browser session. */
export function session(chrome, url, profile) {
	return withPage(chrome, url, profile, async (page) => {
		await page.evaluate(call('settled'));
		const heapAtLoad = await heapAfterGc(page);
		const switches = await switching(page);
		const heapAfterSwitching = await heapAfterGc(page);
		const resizes = await resizing(page);
		await page.evaluate(call('switchTo', ROUTES[0].path));
		const idle = await quiet(page);
		return { heapAtLoad, switches, heapAfterSwitching, resizes, idle };
	});
}

/** Every page in turn, ROUNDS times: [{ path, round, firstVisit, ms, longTaskMs }]. */
async function switching(page) {
	const order = [...ROUTES.slice(1), ROUTES[0]];
	const rounds = await repeat(ROUNDS, async (round) => {
		const visits = [];
		for (const route of order) {
			const timing = await page.evaluate(call('switchTo', route.path));
			visits.push({ path: route.path, round, firstVisit: round === 0 && route !== ROUTES[0], ...timing });
		}
		return visits;
	});
	return rounds.flat();
}

/** Each resized page through WIDTHS: [{ path, from, to, ms, layoutMs, scriptMs }]. */
async function resizing(page) {
	const results = [];
	for (const path of RESIZED) {
		await page.evaluate(call('switchTo', path));
		let from = WIDTHS.at(-1);
		for (const to of WIDTHS) {
			const before = await page.metrics();
			const start = performance.now();
			await page.resize(to, HEIGHT);
			await page.evaluate(call('settled'));
			const ms = performance.now() - start;
			const after = await page.metrics();
			const spent = (name) => (after[name] - before[name]) * 1000;
			results.push({ path, from, to, ms, layoutMs: spent('LayoutDuration') + spent('RecalcStyleDuration'), scriptMs: spent('ScriptDuration') });
			from = to;
		}
	}
	return results;
}

/** IDLE_S seconds of nothing on the overview: { cpuPercent, taskMs, requests, heapBytes, heapGrowthBytes, nodes }. */
async function quiet(page) {
	await new Promise((resolve) => setTimeout(resolve, 2000));
	let requests = 0;
	page.on('Network.requestWillBeSent', () => (requests += 1));
	const before = await page.metrics();
	await new Promise((resolve) => setTimeout(resolve, IDLE_S * 1000));
	const after = await page.metrics();
	const taskMs = (after.TaskDuration - before.TaskDuration) * 1000;
	return {
		cpuPercent: (taskMs / (IDLE_S * 1000)) * 100,
		taskMs,
		requests,
		heapBytes: after.JSHeapUsedSize,
		heapGrowthBytes: after.JSHeapUsedSize - before.JSHeapUsedSize,
		nodes: after.Nodes
	};
}

/** The JavaScript heap in use once garbage is collected, in bytes. */
async function heapAfterGc(page) {
	await page.send('HeapProfiler.collectGarbage');
	return (await page.metrics()).JSHeapUsedSize;
}
