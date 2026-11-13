// The pure pieces behind the timing reports: process readings, process families, summaries of
// timings, the environment profiles, and which reports a selection runs.
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { exportLines, profileVars } from '../../env.mjs';
import { family, parseTable, shortName } from '../lib/app-processes.mjs';
import { linuxCpuMs, linuxRssBytes, psTimeMs } from '../lib/process.mjs';
import { select } from '../lib/registry.mjs';
import { median, percentile, summary } from '../lib/stats.mjs';

test('ps CPU time: minutes and seconds, hours, days', () => {
	assert.equal(psTimeMs('0:00.05'), 50);
	assert.equal(psTimeMs('1:02.50'), 62_500);
	assert.equal(psTimeMs('01:00:00'), 3_600_000);
	assert.equal(psTimeMs('2-00:00:01'), 172_801_000);
});

test('Linux /proc readings: utime + stime after a command with spaces, VmRSS', () => {
	const stat = '4242 (my app (x)) S 1 2 3 4 5 6 7 8 9 10 150 50 0 0';
	assert.equal(linuxCpuMs(stat), 2000);
	assert.equal(linuxRssBytes('Name:\tx\nVmRSS:\t  2048 kB\n'), 2048 * 1024);
	assert.equal(linuxRssBytes('Name:\tx\n'), null);
});

test('timings: nearest-rank percentiles, the median, a summary of none', () => {
	assert.equal(median([30, 10, 20]), 20);
	assert.equal(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.95), 10);
	assert.equal(percentile([], 0.5), null);
	assert.deepEqual(summary([]), { median: null, max: null, runs: 0 });
	assert.deepEqual(summary([5, 1, 9]), { median: 5, max: 9, runs: 3 });
});

test('an app\'s processes: itself, its descendants, and webview helpers started after the snapshot', () => {
	const table = parseTable(`
	  100     1 /Apps/gitprompt-desktop
	  101   100 /Apps/gitprompt
	  102   101 /bin/sh -c git status
	  200     1 /System/Library/Frameworks/WebKit.framework/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent
	  201     1 /System/Library/Frameworks/WebKit.framework/XPCServices/com.apple.WebKit.Networking.xpc/Contents/MacOS/com.apple.WebKit.Networking
	  300     1 /usr/bin/unrelated`);
	const members = family(100, new Set([201]), table);
	assert.deepEqual(
		members.map((m) => m.pid),
		[100, 101, 102, 200]
	);
	assert.equal(members.at(-1).name, 'com.apple.WebKit.WebContent');
	assert.equal(shortName('/a/b/gitprompt-desktop --hidden'), 'gitprompt-desktop');
});

test('profiles layer .env, .env.local, .env.<profile>, .env.<profile>.local; the shell wins on export', async () => {
	const root = await mkdtemp(join(tmpdir(), 'gitprompt-env-'));
	await writeFile(join(root, '.env'), 'A=base\nB=base\n');
	await writeFile(join(root, '.env.local'), 'B=local\n');
	await writeFile(join(root, '.env.performance'), 'GITPROMPT_ENV=performance\nC=profile\nB=profile\n');
	await writeFile(join(root, '.env.performance.local'), "C=mine's\n");
	const vars = profileVars('performance', root);
	assert.deepEqual(vars, { A: 'base', B: 'profile', GITPROMPT_ENV: 'performance', C: "mine's" });
	const lines = exportLines(vars, { A: 'shell' }).split('\n').sort();
	assert.deepEqual(lines, ["export B='profile'", "export C='mine'\\''s'", "export GITPROMPT_ENV='performance'"]);
	assert.throws(() => profileVars('staging', root), /unknown profile staging/);
});

test('an opt-in report runs only when named, never with its area or with everything', () => {
	const reports = [
		{ id: 'backend/lint', area: 'backend', name: 'lint' },
		{ id: 'backend/desktop', area: 'backend', name: 'desktop', optIn: true }
	];
	const ids = (selectors) => select(reports, selectors).map((r) => r.id);
	assert.deepEqual(ids([]), ['backend/lint']);
	assert.deepEqual(ids(['backend']), ['backend/lint']);
	assert.deepEqual(ids(['desktop']), ['backend/desktop']);
	assert.deepEqual(ids(['backend', 'backend/desktop']), ['backend/lint', 'backend/desktop']);
	assert.throws(() => select(reports, ['nope']), /no report matches nope/);
});
