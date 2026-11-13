// One launch of the desktop app in its performance profile (backend/apps/desktop/src/perf): it
// opens its window, times its own milestones and the dashboard in the real webview, writes them to
// a JSON file, stays open `idleSeconds` untouched (measured from here: the app and its webview
// processes), then quits.
import { spawn } from 'node:child_process';
import { readFile, rm } from 'node:fs/promises';

import { familyIdle, webviewPids } from './app-processes.mjs';

/** How long a launch may take to write its results. */
const TIMEOUT_MS = 180_000;
/** Seconds after the results before idle is measured, and kept free at its end (the app quits then). */
const SETTLE_S = 2;
const MARGIN_S = 1;

/**
 * Launches `binary` once. options: { env (the profile's variables), home, port, dashboard, out,
 * cold, idleSeconds, log(text) }. Resolves { spawnedAt (epoch ms), report, idle } (idle null unless
 * idleSeconds); rejects when the app fails, with what it wrote to stderr.
 */
export async function launchApp(binary, { env, home, port, dashboard, out, cold, idleSeconds = 0, log = () => {} }) {
	await rm(out, { force: true });
	const before = webviewPids();
	const spawnedAt = performance.timeOrigin + performance.now();
	const child = spawn(binary, [], {
		env: {
			...process.env,
			...env,
			GITPROMPT_HOME: home,
			GITPROMPT_PORT: String(port),
			GITPROMPT_DASHBOARD: dashboard,
			GITPROMPT_PERF_OUT: out,
			GITPROMPT_PERF_COLD: String(cold),
			GITPROMPT_PERF_IDLE_SECONDS: String(idleSeconds)
		},
		stdio: ['ignore', 'ignore', 'pipe']
	});
	let stderr = '';
	child.stderr.on('data', (chunk) => {
		stderr += chunk;
		log(String(chunk));
	});
	const exited = new Promise((resolve) => child.once('exit', (code) => resolve(code ?? 1)));
	try {
		const report = await results(out, exited, () => stderr);
		const idle = idleSeconds > 0 ? await idleAfter(child.pid, before, idleSeconds) : null;
		await within(exited, (idleSeconds + 30) * 1000, 'the app did not quit after its run');
		return { spawnedAt, report, idle };
	} finally {
		if (child.exitCode === null && child.signalCode === null) child.kill();
	}
}

/** The results file once the app wrote it; rejects when the app exits first or reports an error. */
async function results(out, exited, stderr) {
	let code = null;
	exited.then((c) => (code = c));
	const deadline = Date.now() + TIMEOUT_MS;
	while (Date.now() < deadline) {
		const text = await readFile(out, 'utf8').catch(() => null);
		if (text) {
			const report = JSON.parse(text);
			if (report.error) throw new Error(`the app's performance run failed: ${report.error}`);
			return report;
		}
		if (code !== null) throw new Error(`the app exited (${code}) without results:\n${stderr().slice(-2000)}`);
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	throw new Error(`the app wrote no results within ${TIMEOUT_MS / 1000} s`);
}

async function idleAfter(pid, before, idleSeconds) {
	await new Promise((resolve) => setTimeout(resolve, SETTLE_S * 1000));
	return familyIdle(pid, before, Math.max(1, idleSeconds - SETTLE_S - MARGIN_S));
}

function within(promise, ms, message) {
	let timer;
	const timeout = new Promise((_, reject) => (timer = setTimeout(() => reject(new Error(message)), ms)));
	return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}
