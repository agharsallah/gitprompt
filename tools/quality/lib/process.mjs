// Processes the measurement reports start and watch: a free port, waiting for a server to answer,
// a daemon started and stopped on a data folder, and what a process costs (CPU time, memory).
import { spawn, spawnSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:net';

/** Clock ticks per second of /proc/<pid>/stat on Linux (USER_HZ, 100 on every common kernel). */
const LINUX_HZ = 100;

export function freePort() {
	return new Promise((resolve, reject) => {
		const server = createServer();
		server.on('error', reject);
		server.listen(0, '127.0.0.1', () => {
			const { port } = server.address();
			server.close(() => resolve(port));
		});
	});
}

/** Resolves once `url` answers 2xx; rejects after timeoutMs. Polls every intervalMs. */
export async function ready(url, { timeoutMs = 60_000, intervalMs = 300 } = {}) {
	const until = Date.now() + timeoutMs;
	while (Date.now() < until) {
		// One attempt never waits long: a request the daemon took while starting may never be answered.
		const ok = await fetch(url, { signal: AbortSignal.timeout(2_000) }).then(
			(response) => response.ok,
			() => false
		);
		if (ok) return;
		await new Promise((resolve) => setTimeout(resolve, intervalMs));
	}
	throw new Error(`nothing answered ${url} within ${timeoutMs / 1000} s`);
}

/**
 * `binary serve` on a free loopback port with `home` as its data folder and `dashboard` as the
 * page it serves. Resolves { url, pid, startMs, stop() } once /api/status answers; startMs is the
 * time from spawn to that answer (polled every 10 ms). stop() sends Ctrl-C and resolves on exit.
 * timeoutMs: how long a start may take (a busy machine and a debug build make it long). A spool is
 * drained after it answers: wait for spoolDrained when the data must be complete.
 */
export async function startDaemon(binary, { home, dashboard, env = {}, timeoutMs = 60_000 }) {
	const port = await freePort();
	const url = `http://127.0.0.1:${port}`;
	const started = performance.now();
	const child = spawn(binary, ['serve'], {
		env: { ...process.env, ...env, GITPROMPT_HOME: home, GITPROMPT_PORT: String(port), GITPROMPT_DASHBOARD: dashboard },
		stdio: 'ignore'
	});
	const exited = new Promise((resolve) => child.once('exit', resolve));
	const stop = () => {
		if (child.exitCode === null && child.signalCode === null) child.kill('SIGINT');
		return exited;
	};
	try {
		await Promise.race([ready(`${url}/api/status`, { timeoutMs, intervalMs: 10 }), exited.then((code) => Promise.reject(new Error(`the daemon exited (${code}) before it answered`)))]);
	} catch (error) {
		await stop();
		throw error;
	}
	return { url, pid: child.pid, startMs: performance.now() - started, stop };
}

/**
 * Resolves once the daemon at `url` has no spooled hook payload left (`spool` of /api/status is
 * empty): it answers first and drains the spool behind. Rejects after timeoutMs.
 */
export async function spoolDrained(url, { timeoutMs = 300_000, intervalMs = 50 } = {}) {
	const until = Date.now() + timeoutMs;
	while (Date.now() < until) {
		const status = await fetch(`${url}/api/status`).then((response) => response.json());
		if (status.spool.length === 0) return;
		await new Promise((resolve) => setTimeout(resolve, intervalMs));
	}
	throw new Error(`the daemon at ${url} still had spooled payloads after ${timeoutMs / 1000} s`);
}

/**
 * What a process has used so far: { cpuMs (user + system), rssBytes }, or null where this platform
 * gives no cheap reading (Windows). Linux reads /proc (ps rounds CPU time to seconds there).
 */
export async function usage(pid) {
	if (process.platform === 'linux') {
		const [stat, status] = await Promise.all([readFile(`/proc/${pid}/stat`, 'utf8'), readFile(`/proc/${pid}/status`, 'utf8')]);
		return { cpuMs: linuxCpuMs(stat), rssBytes: linuxRssBytes(status) };
	}
	if (process.platform === 'darwin') {
		const ps = spawnSync('ps', ['-o', 'time=,rss=', '-p', String(pid)], { encoding: 'utf8' });
		if (ps.status !== 0) return null;
		const [time, rss] = ps.stdout.trim().split(/\s+/);
		return { cpuMs: psTimeMs(time), rssBytes: Number(rss) * 1024 };
	}
	return null;
}

/** ps's `time` column ([[dd-]hh:]mm:ss[.cc]) in milliseconds. */
export function psTimeMs(text) {
	const [days, clock] = text.includes('-') ? text.split('-') : ['0', text];
	const seconds = clock
		.split(':')
		.map(Number)
		.reduce((total, part) => total * 60 + part, 0);
	return Math.round((Number(days) * 86_400 + seconds) * 1000);
}

/** utime + stime of a /proc/<pid>/stat line in milliseconds (fields after the command, which may hold spaces). */
export function linuxCpuMs(stat) {
	const fields = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
	// fields[0] is the state (field 3); utime and stime are fields 14 and 15.
	return ((Number(fields[11]) + Number(fields[12])) * 1000) / LINUX_HZ;
}

/** VmRSS of /proc/<pid>/status in bytes. */
export function linuxRssBytes(status) {
	const match = status.match(/^VmRSS:\s+(\d+)\s+kB/m);
	return match ? Number(match[1]) * 1024 : null;
}

/**
 * CPU a process used while idle for `seconds`: { cpuPercent (of one core), cpuMs, rssBytes (at
 * the end) }, or null without a reading.
 */
export async function idleUsage(pid, seconds) {
	const before = await usage(pid);
	if (!before) return null;
	await new Promise((resolve) => setTimeout(resolve, seconds * 1000));
	const after = await usage(pid);
	const cpuMs = after.cpuMs - before.cpuMs;
	return { cpuMs, cpuPercent: (cpuMs / (seconds * 1000)) * 100, rssBytes: after.rssBytes };
}
