// What GitPrompt's performance checks measure inside the dashboard, run before the page's own
// scripts: by the desktop app in its performance profile (src/perf, in the system webview) and by
// `just quality` in headless Chrome (tools/quality/lib/page-probes.mjs). It defines
// window.__gitpromptPerf:
//   settled()        resolves performance.now() once the page shows its data and painted: <main>
//                    holds content, no navigation in flight, nothing loading on screen.
//   firstLoad()      the load's timings (FCP, LCP, settled, DOMContentLoaded, load), requests,
//                    bytes, long tasks and DOM nodes.
//   switchTo(path)   clicks the sidebar link to `path`; { ms, longTaskMs } until the new page settled.
//   resized(width)   once the viewport is `width` wide and the page painted again: { paintedAt }
//                    (epoch ms, to compare with the clock of whoever resized the window).
//   inspect()        { nodes, heapBytes } now.
//   start(id, name, ...args) / take(id)  run a probe and fetch its result later, for callers that
//                    cannot await a promise (the desktop app): take is null until it is done.
(() => {
	const longTasks = [];
	let fcp = null;
	let lcp = null;
	const supported = (type) => (window.PerformanceObserver?.supportedEntryTypes || []).includes(type);
	const observe = (type, onEntries) => {
		if (supported(type)) new PerformanceObserver((list) => onEntries(list.getEntries())).observe({ type, buffered: true });
	};
	observe('longtask', (entries) => longTasks.push(...entries.map((e) => ({ start: e.startTime, ms: e.duration }))));
	observe('paint', (entries) => (fcp = entries.find((e) => e.name === 'first-contentful-paint')?.startTime ?? fcp));
	observe('largest-contentful-paint', (entries) => (lcp = entries[entries.length - 1].startTime));

	const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
	const painted = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
	const epoch = () => performance.timeOrigin + performance.now();
	/** Polls `done` every few milliseconds; throws `what` after timeoutMs. */
	async function until(done, what, timeoutMs = 20_000) {
		const deadline = performance.now() + timeoutMs;
		while (!done()) {
			if (performance.now() > deadline) throw new Error(`${what} within ${timeoutMs / 1000} s`);
			await sleep(2);
		}
	}
	// Only what is on screen counts: panels below the fold load when scrolled near (lib/async/Lazy.svelte).
	const onScreen = (element) => {
		const box = element.getBoundingClientRect();
		return box.bottom > 0 && box.top < window.innerHeight && box.width > 0;
	};
	const busy = () => {
		const main = document.querySelector('main');
		if (!main || main.childElementCount === 0 || main.classList.contains('loading')) return true;
		if (document.querySelector('.progress')) return true;
		return [...main.querySelectorAll('[aria-busy="true"], .skeleton')].some(onScreen);
	};
	const longTaskMs = (since) => longTasks.filter((t) => t.start >= since).reduce((total, t) => total + t.ms, 0);
	// The dashboard's paths end with a slash (/prompts/); the probes take them without.
	const pathOf = (url) => new URL(url, location.href).pathname.replace(/(.)\/$/, '$1');
	const inspect = () => ({ nodes: document.getElementsByTagName('*').length, heapBytes: performance.memory?.usedJSHeapSize ?? null });

	async function settled() {
		await until(() => !busy(), `${location.pathname} did not settle`);
		// A frame that never comes (a hidden window) must not hang the check.
		await Promise.race([painted(), sleep(1000).then(() => Promise.reject(new Error('no frame painted within 1 s')))]);
		return performance.now();
	}

	async function firstLoad() {
		const ready = await settled();
		// Paint entries arrive a little after the frame they time; a browser without them reports none.
		const paintsDone = () => (fcp !== null || !supported('paint')) && (lcp !== null || !supported('largest-contentful-paint'));
		await until(paintsDone, 'no paint was reported', 2000).catch(() => {});
		const [nav] = performance.getEntriesByType('navigation');
		const resources = performance.getEntriesByType('resource');
		return {
			timeOrigin: performance.timeOrigin,
			fcp,
			lcp,
			ready,
			domContentLoaded: nav.domContentLoadedEventEnd,
			load: nav.loadEventEnd,
			requests: resources.length + 1,
			transferBytes: resources.reduce((total, r) => total + (r.transferSize || 0), nav.transferSize || 0),
			longTaskMs: supported('longtask') ? longTaskMs(0) : null,
			...inspect()
		};
	}

	async function switchTo(path) {
		const link = [...document.querySelectorAll('nav[aria-label="Main"] a')].find((a) => pathOf(a.href) === path);
		if (!link) throw new Error(`no sidebar link to ${path}`);
		const start = performance.now();
		link.click();
		await until(() => pathOf(location.href) === path, `the click did not open ${path}`);
		const end = await settled();
		return { ms: end - start, longTaskMs: supported('longtask') ? longTaskMs(start) : null };
	}

	async function resized(width) {
		await until(() => window.innerWidth === width, `the window did not become ${width} px wide`);
		await settled();
		return { paintedAt: epoch() };
	}

	const probes = { settled, firstLoad, switchTo, resized, inspect };
	const results = new Map();
	window.__gitpromptPerf = {
		...probes,
		start(id, name, ...args) {
			Promise.resolve()
				.then(() => probes[name](...args))
				.then(
					(value) => results.set(id, { value }),
					(error) => results.set(id, { error: String(error?.message || error) })
				);
		},
		take(id) {
			const result = results.get(id) ?? null;
			results.delete(id);
			return result;
		}
	};
})();
