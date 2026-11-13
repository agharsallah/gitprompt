// One build at a time: a second build refuses while the first lives, and a dead build's lock is taken over.
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, test } from 'node:test';

import { takeLock } from '../lib/lock.mjs';

const dir = mkdtempSync(join(tmpdir(), 'quality-lock-'));
after(() => rmSync(dir, { recursive: true, force: true }));

test('a second build sees the first one holding the lock, and gets it once released', () => {
	const path = join(dir, 'build.lock');
	const first = takeLock(path, ['web']);
	assert.ok(first.release);
	const second = takeLock(path, ['all']);
	assert.equal(second.held.pid, process.pid);
	assert.deepEqual(second.held.args, ['web']);
	first.release();
	assert.equal(existsSync(path), false);
	const third = takeLock(path);
	assert.ok(third.release);
	third.release();
});

test('the lock of a build that is gone (killed) is taken over', () => {
	const path = join(dir, 'dead.lock');
	// No process has a pid this high on macOS or Linux.
	writeFileSync(path, JSON.stringify({ pid: 99_999_999, started_at: 0, args: [] }));
	const lock = takeLock(path);
	assert.ok(lock.release);
	lock.release();
});

test('an unreadable lock counts as held while young (a build is writing it), as stale once old', () => {
	const path = join(dir, 'half.lock');
	writeFileSync(path, '');
	assert.deepEqual(takeLock(path).held, { pid: null });
	const old = new Date(Date.now() - 60_000);
	utimesSync(path, old, old);
	const lock = takeLock(path);
	assert.ok(lock.release);
	lock.release();
});
