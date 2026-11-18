import { describe, expect, it } from 'vitest';
import { calmLine, periodName } from '#lib/overview/calm.ts';
import { parseRange } from '#lib/insights/range.ts';
import { bucket } from '#lib/testing/builders.ts';

describe('calmLine', () => {
	it('says the period in one line: prompts, spend and commits', () => {
		const t = bucket({ prompts: 278, cost_usd: 453, git_commits: 181 });
		expect(calmLine(parseRange('7d'), t)).toBe('Last 7 days: 278 prompts · $453 · 181 commits');
	});

	it('leaves out what is zero, and says when there were no prompts', () => {
		expect(calmLine(parseRange('24h'), bucket({ prompts: 1 }))).toBe('Last 24 hours: 1 prompt');
		expect(calmLine(parseRange('all'), bucket())).toBe('All time: no prompts');
	});
});

describe('periodName', () => {
	it('names the window', () => expect(periodName(parseRange('30d'))).toBe('Last 30 days'));
});
