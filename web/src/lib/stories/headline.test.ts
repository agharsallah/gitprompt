import { describe, expect, it } from 'vitest';
import type { PreviousPeriod, Stats } from '#lib/api/types.ts';
import { bucket, changes, delta, redone, stats } from '#lib/testing/builders.ts';
import { movement, overviewHeadline, periodName, totalsLine } from './headline.ts';

/** Key-figure stats with these changes against the 7 days before. */
function week(compared: Partial<PreviousPeriod['changes']>, extra: Partial<Stats> = {}): Stats {
	return stats({
		...extra,
		previous: { since: '', until: '', totals: bucket(), changes: changes(compared) }
	});
}

const headline = (s: Stats, top?: { name: string; cost_usd: number }) =>
	overviewHeadline({ stats: s, range: '7d', top });

describe('overviewHeadline', () => {
	it('names spend that doubled, and the project that spent most of it', () => {
		const s = week({ cost_usd: delta(120, 60) }, { totals: bucket({ cost_usd: 120 }) });
		expect(headline(s, { name: 'gitprompt', cost_usd: 80 })).toBe(
			'Spend doubled this week, mostly gitprompt'
		);
		expect(headline(s, { name: 'gitprompt', cost_usd: 20 })).toBe('Spend doubled this week');
	});

	it('names agent working time that halved', () => {
		const s = week(
			{ agent_time_s: delta(7200, 14400) },
			{ totals: bucket({ agent_time_s: 7200 }) }
		);
		expect(headline(s)).toBe('Agent working time halved this week: 2h 00m');
	});

	it('names redone work with its cost, or its time alone without cost', () => {
		const work = redone({ turns: 4, agent_time_s: 10800, cost_usd: 38 });
		const s = week({ redone_agent_time_s: delta(10800, 3600) }, { redone: work });
		expect(headline(s)).toBe('Redone work tripled this week: $38.00 and 3h 00m of agent time');
		const timeOnly = week(
			{ redone_agent_time_s: delta(10800, 3600) },
			{ redone: redone({ turns: 4, agent_time_s: 10800 }) }
		);
		expect(headline(timeOnly)).toBe('Redone work tripled this week: 3h 00m of agent time');
	});

	it('names commits up by a share, and leaks up many times', () => {
		const commits = week({ git_commits: delta(16, 10) }, { totals: bucket({ git_commits: 16 }) });
		expect(headline(commits)).toBe('Agent commits up 60% this week: 16');
		const leaks = week({ possible_leaks: delta(6, 1) }, { possible_leaks: 6 });
		expect(headline(leaks)).toBe('Possible leaks up 6× this week: 6 open');
	});

	it('picks the larger change when two stand out', () => {
		const s = week({ cost_usd: delta(20, 10), git_commits: delta(40, 10) });
		expect(headline(s)).toMatch(/^Agent commits up 4×/);
	});

	it('says nothing when no figure moved enough, or moved on too small a base', () => {
		expect(headline(week({ cost_usd: delta(110, 100) }))).toBeNull();
		expect(headline(week({ cost_usd: delta(3, 1) }))).toBeNull();
		expect(headline(week({}))).toBeNull();
	});

	it('says nothing without a period before', () => {
		expect(headline(stats())).toBeNull();
		expect(headline(week({ cost_usd: delta(50) }))).toBeNull();
	});

	it('names the period of the range', () => {
		const s = week({ cost_usd: delta(20, 10) });
		expect(overviewHeadline({ stats: s, range: '24h' })).toBe('Spend doubled today');
		expect(periodName('30d')).toBe('this month');
	});
});

describe('movement', () => {
	it.each([
		[0.6, 'up 60%'],
		[1, 'doubled'],
		[2, 'tripled'],
		[4, 'up 5×'],
		[-0.5, 'halved'],
		[-0.8, 'down 80%'],
		[-0.2, 'down 20%']
	])('%d reads "%s"', (change, words) => expect(movement(change)).toBe(words));
});

describe('totalsLine', () => {
	it('lists prompts, sessions, spend and commits', () => {
		const t = bucket({ prompts: 278, sessions: 41, cost_usd: 453, git_commits: 181 });
		expect(totalsLine(t)).toBe('278 prompts · 41 sessions · $453 · 181 commits');
	});

	it('leaves out spend and commits there are none of', () => {
		expect(totalsLine(bucket({ prompts: 1, sessions: 1 }))).toBe('1 prompt · 1 session');
	});
});
