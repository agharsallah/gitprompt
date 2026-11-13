// One build at a time: two builds would share the coverage folder, backend/target and status.js,
// break each other's commands and show the dashboard both in turn. A build takes target/quality/
// build.lock (its pid, start and arguments); another one refuses while that process lives, and takes
// over a lock its process left behind (killed).
import { closeSync, openSync, readFileSync, statSync, unlinkSync, writeSync } from 'node:fs';

/** A lock file this young with nothing readable in it is being written by a build starting now. */
const WRITING_MS = 5_000;

/** Whether a process with this pid runs (and is not ours to doubt: EPERM means it exists). */
function alive(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		return error.code === 'EPERM';
	}
}

function holder(path) {
	try {
		return JSON.parse(readFileSync(path, 'utf8'));
	} catch {
		return null;
	}
}

/**
 * Takes the lock at `path` for this process: resolves { release } , or { held: { pid, started_at,
 * args } } when a live build holds it. A lock whose process is gone is taken over.
 */
export function takeLock(path, args = []) {
	for (let attempt = 0; attempt < 2; attempt += 1) {
		try {
			const fd = openSync(path, 'wx');
			writeSync(fd, JSON.stringify({ pid: process.pid, started_at: Date.now(), args }));
			closeSync(fd);
			let released = false;
			return {
				release() {
					if (released) return;
					released = true;
					if (holder(path)?.pid === process.pid) unlinkSync(path);
				}
			};
		} catch (error) {
			if (error.code !== 'EEXIST') throw error;
			const current = holder(path);
			if (current && alive(current.pid)) return { held: current };
			if (!current && Date.now() - statSync(path).mtimeMs < WRITING_MS) return { held: { pid: null } };
			unlinkSync(path);
		}
	}
	return { held: holder(path) ?? { pid: null } };
}
