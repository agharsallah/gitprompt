// The scheduler's rules: a place limit, `after`, longest first, and measurements alone.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { gate, schedule } from '../lib/scheduler.mjs';

const tick = (ms = 5) => new Promise((resolve) => setTimeout(resolve, ms));

/** Runs reports that each take `ms`, recording start order and the most running at once. */
async function runAll(reports, { jobs, expected }) {
	const started = [];
	let running = 0;
	let most = 0;
	await schedule(reports, {
		jobs,
		expected,
		runReport: async (report, slots) => {
			started.push(report.id);
			running += 1;
			most = Math.max(most, running);
			await tick(report.ms ?? 5);
			running -= 1;
			slots.release();
		}
	});
	return { started, most };
}

test('never runs more reports at once than jobs', async () => {
	const reports = Array.from({ length: 7 }, (_, i) => ({ id: `r${i}` }));
	const { started, most } = await runAll(reports, { jobs: 3 });
	assert.equal(most, 3);
	assert.equal(started.length, 7);
});

test('starts the longest report first, and one never built before all of them', async () => {
	const last = { short: 1_000, long: 90_000, middle: 30_000 };
	const reports = ['short', 'new', 'long', 'middle'].map((id) => ({ id }));
	const { started } = await runAll(reports, { jobs: 1, expected: (report) => last[report.id] });
	assert.deepEqual(started, ['new', 'long', 'middle', 'short']);
});

test('a report waits for the reports of its `after` in the same build, and only those', async () => {
	const reports = [
		{ id: 'architecture', after: ['tests', 'absent'] },
		{ id: 'tests', ms: 30 }
	];
	const { started } = await runAll(reports, { jobs: 4, expected: (report) => (report.id === 'architecture' ? 100 : 1) });
	assert.deepEqual(started, ['tests', 'architecture']);
});

test('reports waiting for each other in a loop still run', async () => {
	const reports = [
		{ id: 'a', after: ['b'] },
		{ id: 'b', after: ['a'] }
	];
	const { started } = await runAll(reports, { jobs: 2 });
	assert.equal(started.length, 2);
});

test('a failing report does not stop the others', async () => {
	const ran = [];
	await schedule([{ id: 'bad' }, { id: 'good' }], {
		jobs: 1,
		runReport: async (report, slots) => {
			slots.release();
			ran.push(report.id);
			if (report.id === 'bad') throw new Error('boom');
		}
	});
	assert.deepEqual(ran.sort(), ['bad', 'good']);
});

test('a measurement waits for running commands, and pauses the ones that start meanwhile', async () => {
	const lock = gate();
	const events = [];
	const command = (name, ms) =>
		lock.shared(async () => {
			events.push(`${name} start`);
			await tick(ms);
			events.push(`${name} end`);
		});
	const first = command('build', 20);
	await tick(1);
	const measured = lock.alone(async () => {
		events.push('measure start');
		assert.equal(lock.measuring(), true);
		await tick(20);
		events.push('measure end');
	});
	await tick(30);
	const later = command('lint', 5);
	await Promise.all([first, measured, later]);
	assert.deepEqual(events, ['build start', 'build end', 'measure start', 'measure end', 'lint start', 'lint end']);
	assert.equal(lock.measuring(), false);
});

test('a measurement waits while other work is left, and starts when poked once there is none', async () => {
	let othersLeft = true;
	const lock = gate(() => !othersLeft);
	let measured = false;
	const measurement = lock.alone(async () => {
		measured = true;
	});
	await tick();
	lock.poke();
	assert.equal(measured, false);
	othersLeft = false;
	lock.poke();
	await measurement;
	assert.equal(measured, true);
});
