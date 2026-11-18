import { describe, expect, it } from 'vitest';
import { queryParams } from '#lib/api/query.ts';
import type { PromptQuery, Stats } from '#lib/api/types.ts';
import { parseRange } from '#lib/insights/range.ts';
import type { Availability } from '#lib/sources/capabilities.ts';
import { bucket, changes, delta, redone, stats } from '#lib/testing/builders.ts';
import {
	changeWords,
	keyFigures,
	redoneValue,
	topProject,
	type Figure,
	type FigureId,
	type KeyFigureInput
} from './keyFigures.ts';

const reports = (tool = 'Claude Code'): Availability => ({
	capability: 'cost',
	available: [tool],
	connectable: [],
	updatable: [],
	unavailable: []
});
const missing: Availability = { ...reports(), available: [], unavailable: ['Cursor'] };

const week = parseRange('7d');
const link = (extra: PromptQuery = {}) => `/prompts?${queryParams(extra)}`;

function input(overrides: Partial<KeyFigureInput> = {}): KeyFigureInput {
	return {
		stats: stats(),
		repos: stats(),
		range: week,
		slots: [],
		link,
		leaksHref: '/',
		can: { cost: reports(), turnEnd: reports(), tools: reports() },
		...overrides
	};
}

const figure = (id: FigureId, i: KeyFigureInput): Figure =>
	keyFigures(i).find((f) => f.id === id) ?? { id, label: '', value: '' };

describe('changeWords', () => {
	it('says how a figure moved against the range before', () => {
		expect(changeWords(delta(118, 100), week)).toBe('+18% vs the 7 days before');
		expect(changeWords(delta(50, 100), parseRange('24h'))).toBe('−50% vs the 24 hours before');
		expect(changeWords(delta(5, 5), week)).toBe('same as the 7 days before');
		expect(changeWords(delta(5, 0), week)).toBe('none the 7 days before');
	});

	it('says nothing without a period before', () => {
		expect(changeWords(undefined, week)).toBeUndefined();
		expect(changeWords(delta(5), week)).toBeUndefined();
		expect(changeWords(delta(0, 0), week)).toBeUndefined();
		expect(changeWords(delta(118, 100), parseRange('all'))).toBeUndefined();
	});
});

describe('keyFigures', () => {
	it('shows the five, in order', () => {
		expect(keyFigures(input()).map((f) => f.id)).toEqual([
			'redone',
			'agent_time',
			'spend',
			'commits',
			'leaks'
		]);
	});

	it('words redone work as cost, split by reason, amber, linking to failed and interrupted turns', () => {
		const work = redone({
			turns: 3,
			agent_time_s: 4320,
			cost_usd: 14.2,
			failed: { turns: 2, agent_time_s: 2400, cost_usd: 10.1 },
			reworded: { turns: 1, agent_time_s: 1920, cost_usd: 4.1 }
		});
		const s = stats({
			redone: work,
			previous: {
				since: '',
				until: '',
				totals: bucket(),
				changes: changes({ redone_cost_usd: delta(14.2, 7.1) })
			}
		});
		const f = figure('redone', input({ stats: s }));
		expect(f.value).toBe('$14.20 and 1h 12m redone');
		expect(f.hint).toBe('failed $10.10 · reworded $4.10');
		expect(f.title).toContain('failed: 2 turns, 40m 00s, $10.10');
		expect(f.change).toBe('+100% vs the 7 days before');
		expect(f.tone).toBe('warning');
		expect(f.href).toBe('/prompts?outcome=failed&outcome=interrupted');
	});

	it('shows redone time alone without cost', () => {
		expect(redoneValue(redone({ turns: 1, agent_time_s: 600 }))).toBe('10m 00s redone');
	});

	it('says "Nothing redone" for a period with none', () => {
		const f = figure('redone', input({ stats: stats({ redone: redone() }) }));
		expect(f.value).toBe('Nothing redone');
		expect(f.tone).toBeUndefined();
	});

	it('says "Not reported" and who could, never 0, for what no tool reports', () => {
		const i = input({ can: { cost: missing, turnEnd: missing, tools: missing } });
		for (const id of ['redone', 'agent_time', 'spend', 'commits'] as const) {
			expect(figure(id, i).value).toBe('Not reported');
			expect(figure(id, i).hint).toMatch(/^Cursor can’t report/);
		}
	});

	it('names the project that spent most, with spend trend from the slots', () => {
		const repos = stats({ groups: [bucket({ key: '/work/gitprompt', cost_usd: 80 })] });
		const slots = [
			{ key: 'a', date: new Date(0), bucket: bucket({ cost_usd: 2 }) },
			{ key: 'b', date: new Date(0), bucket: null }
		];
		const f = figure(
			'spend',
			input({ repos, slots, stats: stats({ totals: bucket({ cost_usd: 99 }) }) })
		);
		expect(f.value).toBe('$99.00');
		expect(f.hint).toBe('most on gitprompt: $80.00');
		expect(f.trend).toEqual([2, 0]);
	});

	it('counts open possible leaks, and says "None found" without', () => {
		expect(figure('leaks', input()).value).toBe('None found');
		const f = figure('leaks', input({ stats: stats({ possible_leaks: 3 }) }));
		expect(f.value).toBe('3');
		expect(f.tone).toBe('warning');
		expect(f.href).toBe('/');
	});
});

describe('topProject', () => {
	it('skips projects without spend', () => {
		const none: Stats = stats({ groups: [bucket({ key: '/a', cost_usd: 0 })] });
		expect(topProject(none)).toBeUndefined();
		const named = stats({ groups: [bucket({ key: '/a', detail: 'App', cost_usd: 1 })] });
		expect(topProject(named)).toEqual({ name: 'App', cost_usd: 1 });
	});
});
