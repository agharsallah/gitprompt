// Which reports a `just quality …` runs: checks by default, measurements only when asked.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { select } from '../lib/registry.mjs';

const reports = [
	{ id: 'backend/lint', area: 'backend', name: 'lint' },
	{ id: 'backend/performance', area: 'backend', name: 'performance', measure: true },
	{ id: 'backend/desktop', area: 'backend', name: 'desktop', measure: true, optIn: true },
	{ id: 'web/lint', area: 'web', name: 'lint' },
	{ id: 'web/lighthouse', area: 'web', name: 'lighthouse', measure: true }
];
const ids = (...selectors) => select(reports, selectors).map((r) => r.id);

test('no selector and an area run the checks, not the measurements', () => {
	assert.deepEqual(ids(), ['backend/lint', 'web/lint']);
	assert.deepEqual(ids('web'), ['web/lint']);
});

test('`measure` runs every measurement but the opt-in ones; `all` runs checks and measurements', () => {
	assert.deepEqual(ids('measure'), ['backend/performance', 'web/lighthouse']);
	assert.deepEqual(ids('all'), ['backend/lint', 'backend/performance', 'web/lint', 'web/lighthouse']);
	assert.deepEqual(ids('web', 'measure'), ['backend/performance', 'web/lint', 'web/lighthouse']);
});

test('a measurement or an opt-in report runs when named', () => {
	assert.deepEqual(ids('lighthouse'), ['web/lighthouse']);
	assert.deepEqual(ids('backend/desktop'), ['backend/desktop']);
	assert.throws(() => select(reports, ['nope']), /no report matches nope; there are: measure, all, /);
});
