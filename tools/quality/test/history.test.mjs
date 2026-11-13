// Each run's main numbers, kept per report for the Trends page.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, test } from 'node:test';

import { MAX_RUNS, addRun, loadHistory, writeHistory } from '../lib/history.mjs';
import { metric } from '../lib/spec.mjs';

const dir = mkdtempSync(join(tmpdir(), 'quality-history-'));
after(() => rmSync(dir, { recursive: true, force: true }));

const coverage = (value) => [metric('lines', 'Line coverage', value, 'share', 'higher'), metric('failing', 'Failing tests', 0, 'count', 'lower')];

test('a run keeps its numbers and the commit; runs stay in time order', () => {
	const history = { reports: {} };
	addRun(history, 'web/tests', { at: 2, commit: 'b', trend: coverage(0.9) });
	addRun(history, 'web/tests', { at: 1, commit: 'a', trend: coverage(0.8) });
	const entry = history.reports['web/tests'];
	assert.deepEqual(
		entry.runs.map((r) => [r.at, r.commit, r.values.lines]),
		[
			[1, 'a', 0.8],
			[2, 'b', 0.9]
		]
	);
	assert.deepEqual(entry.metrics[0], { key: 'lines', label: 'Line coverage', unit: 'share', better: 'higher' });
});

test('a number that is not finite is left out of its run, and a run with none is not kept', () => {
	const history = { reports: {} };
	assert.equal(addRun(history, 'backend/tests', { at: 1, trend: coverage(null) }), true);
	assert.deepEqual(history.reports['backend/tests'].runs[0].values, { failing: 0 });
	assert.equal(addRun(history, 'web/lint', { at: 1, trend: [metric('findings', 'Findings', Number.NaN, 'count', 'lower')] }), false);
	assert.equal(addRun(history, 'web/bundle', { at: 1 }), false);
	assert.equal(history.reports['web/lint'], undefined);
});

test('a metric the latest run lacks stays defined while older runs have it', () => {
	const history = { reports: {} };
	addRun(history, 'web/tests', { at: 1, trend: coverage(0.8) });
	addRun(history, 'web/tests', { at: 2, trend: [metric('failing', 'Failing tests', 1, 'count', 'lower')] });
	assert.deepEqual(
		history.reports['web/tests'].metrics.map((m) => m.key),
		['failing', 'lines']
	);
});

test('only the latest runs are kept', () => {
	const history = { reports: {} };
	for (let at = 1; at <= MAX_RUNS + 5; at += 1) addRun(history, 'web/lint', { at, trend: [metric('findings', 'Findings', at, 'count', 'lower')] });
	const { runs } = history.reports['web/lint'];
	assert.equal(runs.length, MAX_RUNS);
	assert.equal(runs[0].at, 6);
});

test('written as JSON for the next build and as a script for the page; a missing file is empty', () => {
	const json = join(dir, 'history.json');
	const script = join(dir, 'history.js');
	assert.deepEqual(loadHistory(json), { reports: {} });
	const history = { reports: {} };
	addRun(history, 'web/tests', { at: 1, trend: coverage(0.8) });
	writeHistory(history, { json, script });
	assert.deepEqual(loadHistory(json), history);
	assert.match(readFileSync(script, 'utf8'), /^qualityHistory\(\{"reports":/);
});
