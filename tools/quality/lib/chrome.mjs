// Headless Chrome driven over the DevTools protocol, with no dependency: Node's own WebSocket.
// launchChrome() starts a browser on a profile folder (a fresh one is a cold start, the same one
// again is a relaunch with a warm cache); browser.open(url) gives a page to evaluate scripts in,
// read metrics from and resize.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const CANDIDATES = {
	darwin: ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/Applications/Chromium.app/Contents/MacOS/Chromium', '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary'],
	linux: ['google-chrome', 'google-chrome-stable', 'chromium', 'chromium-browser'],
	win32: [join(process.env.PROGRAMFILES || 'C:\\Program Files', 'Google', 'Chrome', 'Application', 'chrome.exe'), join(process.env['PROGRAMFILES(X86)'] || 'C:\\Program Files (x86)', 'Google', 'Chrome', 'Application', 'chrome.exe')]
};

/** The Chrome to run: CHROME_PATH, else the usual install places; null when there is none. */
export function findChrome() {
	if (process.env.CHROME_PATH) return process.env.CHROME_PATH;
	for (const candidate of CANDIDATES[process.platform] || []) {
		if (candidate.includes('/') || candidate.includes('\\')) {
			if (existsSync(candidate)) return candidate;
		} else if (spawnSync('sh', ['-c', `command -v ${candidate}`]).status === 0) {
			return candidate;
		}
	}
	return null;
}

/**
 * Chrome headless on `profile`, at `width`×`height`. Resolves { open(url, { preload }), close() };
 * preload is a script that runs in the page before its own.
 */
export async function launchChrome(chrome, { profile, width = 1280, height = 840 }) {
	const child = spawn(chrome, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, `--window-size=${width},${height}`, '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-background-networking', '--disable-component-update', '--mute-audio', 'about:blank'], {
		stdio: ['ignore', 'ignore', 'pipe']
	});
	const exited = new Promise((resolve) => child.once('exit', resolve));
	const cdp = await connect(await devtoolsUrl(child));
	return {
		open: (url, { preload } = {}) => openPage(cdp, url, { width, height, preload }),
		async close() {
			await cdp.send('Browser.close').catch(() => child.kill());
			cdp.close();
			await exited;
		}
	};
}

/** The browser's DevTools address, from the line Chrome prints at start. */
function devtoolsUrl(child) {
	return new Promise((resolve, reject) => {
		let seen = '';
		const timer = setTimeout(() => reject(new Error(`Chrome did not start within 30 s:\n${seen.slice(-2000)}`)), 30_000);
		child.stderr.on('data', (chunk) => {
			seen += chunk;
			const match = seen.match(/DevTools listening on (ws:\/\/\S+)/);
			if (match) {
				clearTimeout(timer);
				resolve(match[1]);
			}
		});
		child.once('exit', (code) => reject(new Error(`Chrome exited (${code}) at start:\n${seen.slice(-2000)}`)));
	});
}

/** One WebSocket to the browser; commands of a page go through it with their session id. */
async function connect(url) {
	const socket = new WebSocket(url);
	await new Promise((resolve, reject) => {
		socket.addEventListener('open', resolve, { once: true });
		socket.addEventListener('error', () => reject(new Error(`could not connect to ${url}`)), { once: true });
	});
	let next = 0;
	const pending = new Map();
	const listeners = new Map();
	socket.addEventListener('message', ({ data }) => {
		const message = JSON.parse(data);
		if (message.id === undefined) {
			for (const listener of listeners.get(`${message.sessionId}:${message.method}`) || []) listener(message.params);
			return;
		}
		const { resolve, reject, method } = pending.get(message.id);
		pending.delete(message.id);
		if (message.error) reject(new Error(`${method}: ${message.error.message}`));
		else resolve(message.result);
	});
	return {
		send(method, params = {}, sessionId) {
			next += 1;
			const id = next;
			socket.send(JSON.stringify({ id, method, params, sessionId }));
			return new Promise((resolve, reject) => pending.set(id, { resolve, reject, method }));
		},
		on(sessionId, method, listener) {
			const key = `${sessionId}:${method}`;
			listeners.set(key, [...(listeners.get(key) || []), listener]);
		},
		close: () => socket.close()
	};
}

/**
 * A new tab on `url`, with metrics on, once its load event fired. Resolves the page:
 * { send, on, evaluate(expression), metrics(), resize(width, height) }.
 */
async function openPage(cdp, url, { width, height, preload }) {
	// The tab Chrome opened at start: a tab created beside it runs in the background, where
	// animation frames are paused and paints not reported.
	const { targetInfos } = await cdp.send('Target.getTargets');
	const blank = targetInfos.find((t) => t.type === 'page');
	const targetId = blank ? blank.targetId : (await cdp.send('Target.createTarget', { url: 'about:blank' })).targetId;
	const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
	const page = {
		send: (method, params) => cdp.send(method, params, sessionId),
		on: (method, listener) => cdp.on(sessionId, method, listener),
		/** The value of an expression (a promise is awaited); throws what the page threw. */
		async evaluate(expression) {
			const { result, exceptionDetails } = await page.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
			if (exceptionDetails) throw new Error(`in the page: ${exceptionDetails.exception?.description || exceptionDetails.text}`);
			return result.value;
		},
		/** Chrome's counters as { name: value } (TaskDuration, LayoutDuration, JSHeapUsedSize… in seconds and bytes). */
		async metrics() {
			const { metrics } = await page.send('Performance.getMetrics');
			return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
		},
		resize: (w, h) => page.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false })
	};
	await Promise.all([page.send('Page.enable'), page.send('Performance.enable'), page.send('Network.enable')]);
	await page.send('Page.bringToFront');
	await page.resize(width, height);
	if (preload) await page.send('Page.addScriptToEvaluateOnNewDocument', { source: preload });
	const loaded = new Promise((resolve) => page.on('Page.loadEventFired', resolve));
	await page.send('Page.navigate', { url });
	await loaded;
	return page;
}
