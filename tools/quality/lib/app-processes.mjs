// The processes a desktop app runs as, to add up what it costs: the app, its children, and on
// macOS the WebKit helpers (content, networking, GPU) its webview started, which launchd owns
// rather than the app. Those are told apart by being new: started after the snapshot taken before
// the launch (a Safari tab opened meanwhile would count too).
import { spawnSync } from 'node:child_process';

import { usage } from './process.mjs';

/** Helper processes of the system webview: WebKit's XPC services on macOS, WebKitGTK's on Linux. */
const WEBVIEW = /com\.apple\.WebKit\.|WebKitWebProcess|WebKitNetworkProcess|WebKitGPUProcess/;

/** Every process: [{ pid, ppid, command }]; empty where ps is missing (Windows). */
export function processTable() {
	const ps = spawnSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' });
	if (ps.status !== 0) return [];
	return parseTable(ps.stdout);
}

export function parseTable(text) {
	return text
		.split('\n')
		.map((line) => line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/))
		.filter(Boolean)
		.map(([, pid, ppid, command]) => ({ pid: Number(pid), ppid: Number(ppid), command }));
}

/** The webview helpers running now: a Set of pids, to tell the ones a launch starts. */
export const webviewPids = (table = processTable()) => new Set(table.filter((p) => WEBVIEW.test(p.command)).map((p) => p.pid));

/** The app `pid`, its descendants, and webview helpers not in `before`: [{ pid, name }]. */
export function family(pid, before, table = processTable()) {
	const members = new Set([pid]);
	let grew = true;
	while (grew) {
		grew = false;
		for (const p of table) {
			if (members.has(p.ppid) && !members.has(p.pid)) {
				members.add(p.pid);
				grew = true;
			}
		}
	}
	for (const p of table) if (WEBVIEW.test(p.command) && !before.has(p.pid)) members.add(p.pid);
	return table.filter((p) => members.has(p.pid)).map((p) => ({ pid: p.pid, name: shortName(p.command) }));
}

/** `com.apple.WebKit.WebContent` from its full path, `gitprompt-desktop` from its. */
export function shortName(command) {
	const program = command.split(' ')[0];
	return program.slice(program.lastIndexOf('/') + 1) || command;
}

/**
 * What the app's processes use over `seconds`: { processes: [{ name, cpuPercent, rssBytes }],
 * cpuPercent, rssBytes } summed, or null without readings (Windows).
 */
export async function familyIdle(pid, before, seconds) {
	const members = family(pid, before);
	const start = await readings(members);
	if (start.length === 0) return null;
	await new Promise((resolve) => setTimeout(resolve, seconds * 1000));
	const end = new Map((await readings(members)).map((r) => [r.pid, r]));
	const processes = start
		.filter((r) => end.has(r.pid))
		.map((r) => ({ name: r.name, cpuPercent: ((end.get(r.pid).cpuMs - r.cpuMs) / (seconds * 1000)) * 100, rssBytes: end.get(r.pid).rssBytes }));
	const sum = (key) => processes.reduce((total, p) => total + (p[key] || 0), 0);
	return { processes, cpuPercent: sum('cpuPercent'), rssBytes: sum('rssBytes') };
}

async function readings(members) {
	const all = await Promise.all(members.map(async (m) => ({ ...m, ...(await usage(m.pid).catch(() => null)) })));
	return all.filter((r) => r.cpuMs !== undefined);
}
