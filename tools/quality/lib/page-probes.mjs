// What the runtime report measures inside the dashboard. PROBES is a script Chrome runs in the page
// before the dashboard's own (so it sees every long task), and exposes window.__gitpromptPerf:
//   settled()      resolves performance.now() once the page shows its data and painted: <main>
//                  holds content, no navigation in flight, no skeleton and nothing aria-busy on
//                  screen.
//   firstLoad()    the cold-load timings (FCP, LCP, settled, DOMContentLoaded, load), requests,
//                  bytes transferred and long tasks.
//   switchTo(path) clicks the sidebar link to `path` and times it until the new page settled.
// The page's functions are written here as plain JavaScript and sent as source.

function install() {
	const longTasks = [];
	let fcp = null;
	let lcp = null;
	const observe = (type, onEntries) => {
		try {
			new PerformanceObserver((list) => onEntries(list.getEntries())).observe({ type, buffered: true });
		} catch {
			// A browser without this entry type: the value stays empty.
		}
	};
	observe('longtask', (entries) => longTasks.push(...entries.map((e) => ({ start: e.startTime, ms: e.duration }))));
	observe('paint', (entries) => (fcp = entries.find((e) => e.name === 'first-contentful-paint')?.startTime ?? fcp));
	observe('largest-contentful-paint', (entries) => (lcp = entries[entries.length - 1].startTime));

	const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
	const painted = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
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

	async function settled() {
		await until(() => !busy(), `${location.pathname} did not settle`);
		// A frame that never comes (a hidden tab) must not hang the build.
		await Promise.race([painted(), sleep(1000).then(() => Promise.reject(new Error('no frame painted within 1 s')))]);
		return performance.now();
	}

	async function firstLoad() {
		const ready = await settled();
		// Paint entries arrive a little after the frame they time.
		await until(() => fcp !== null && lcp !== null, 'no paint was reported', 2000).catch(() => {});
		const [nav] = performance.getEntriesByType('navigation');
		const resources = performance.getEntriesByType('resource');
		return {
			fcp,
			lcp,
			ready,
			domContentLoaded: nav.domContentLoadedEventEnd,
			load: nav.loadEventEnd,
			requests: resources.length + 1,
			transferBytes: resources.reduce((total, r) => total + r.transferSize, nav.transferSize),
			longTaskMs: longTaskMs(0)
		};
	}

	// The dashboard's paths end with a slash (/prompts/); the probes take them without.
	const pathOf = (url) => new URL(url, location.href).pathname.replace(/(.)\/$/, '$1');

	async function switchTo(path) {
		const link = [...document.querySelectorAll('nav[aria-label="Main"] a')].find((a) => pathOf(a.href) === path);
		if (!link) throw new Error(`no sidebar link to ${path}`);
		const start = performance.now();
		link.click();
		await until(() => pathOf(location.href) === path, `the click did not open ${path}`);
		const end = await settled();
		return { ms: end - start, longTaskMs: longTaskMs(start) };
	}

	window.__gitpromptPerf = { settled, firstLoad, switchTo };
}

/** The script to preload into the page. */
export const PROBES = `(${install})();`;

/** An expression calling one probe: call('switchTo', '/prompts'). */
export const call = (name, ...args) => `window.__gitpromptPerf.${name}(${args.map((a) => JSON.stringify(a)).join(', ')})`;
