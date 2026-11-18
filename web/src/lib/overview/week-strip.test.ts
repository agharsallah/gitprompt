import { describe, expect, it } from 'vitest';
import { timeSlots } from '#lib/insights/series.ts';
import {
	dayHref,
	median,
	NO_PROJECT,
	OTHER,
	standout,
	standoutLine,
	streakLine,
	stripColumns,
	stripParts,
	stripReadout,
	stripSeries,
	topSeries
} from '#lib/overview/week-strip.ts';
import { sessionsHref } from '#lib/shared/links.ts';
import type { StackedColumn } from '#lib/ui/charts/stacked.ts';
import { bucket, splitDay } from '#lib/testing/builders.ts';

const NOW = new Date(2026, 10, 18, 12);
const column = (key: string, value: number): StackedColumn => ({
	key,
	label: key,
	value,
	parts: []
});

describe('stripSeries: the projects of the period', () => {
	it('ranks projects by prompts, names them, and colors each', () => {
		const series = stripSeries([
			splitDay('2026-11-17', [
				['/w/api', 2],
				['/w/app', 5]
			]),
			splitDay('2026-11-18', [
				['/w/api', 4],
				[null, 1]
			])
		]);
		expect(series).toEqual([
			{ key: '/w/api', label: 'api', color: 'var(--chart-1)' },
			{ key: '/w/app', label: 'app', color: 'var(--chart-2)' },
			{ key: NO_PROJECT, label: 'No project', color: 'var(--chart-3)' }
		]);
	});

	it('stacks the projects beyond the first few as "Other projects"', () => {
		const day = splitDay('2026-11-18', [
			['/a', 5],
			['/b', 4],
			['/c', 1]
		]);
		expect(stripSeries([day], 2).map((s) => s.label)).toEqual(['a', 'b', 'Other projects']);
	});

	it('prefers the name the daemon gives a project', () => {
		const day = bucket({
			key: '2026-11-18',
			parts: [bucket({ key: '/w/x', detail: 'me/x', prompts: 1 })]
		});
		expect(stripSeries([day])[0].label).toBe('me/x');
	});
});

describe('stripParts: stacking in column order', () => {
	it('gives each day its prompts per project in the series order, the rest in Other', () => {
		const days = [
			splitDay('2026-11-17', [
				['/a', 5],
				['/b', 4]
			]),
			splitDay('2026-11-18', [
				['/c', 2],
				['/a', 1]
			])
		];
		const series = stripSeries(days, 2);
		expect(stripParts(days[1], series)).toEqual([
			{ series: '/a', value: 1 },
			{ series: '/b', value: 0 },
			{ series: OTHER, value: 2 }
		]);
		expect(stripParts(null, series).every((p) => p.value === 0)).toBe(true);
	});
});

describe('stripColumns', () => {
	it('lays out every day of the range, quiet ones as zero, each knowing its day', () => {
		const groups = [splitDay('2026-11-17', [['/a', 3]])];
		const slots = timeSlots(groups, new Date(2026, 10, 16), NOW, false);
		const columns = stripColumns(slots, stripSeries(groups), false, NOW);
		expect(columns.map((c) => [c.key, c.value, c.day])).toEqual([
			['2026-11-16', 0, '2026-11-16'],
			['2026-11-17', 3, '2026-11-17'],
			['2026-11-18', 0, '2026-11-18']
		]);
		expect(columns[2].label).toBe('Today');
		expect(columns[1].parts).toEqual([{ series: '/a', value: 3 }]);
	});

	it('names hours over 24 hours, and each hour leads to its day', () => {
		const groups = [splitDay('2026-11-18 11', [['/a', 2]])];
		const slots = timeSlots(groups, new Date(2026, 10, 18, 11), NOW, true);
		const [eleven] = stripColumns(slots, stripSeries(groups), true, NOW);
		expect(eleven.day).toBe('2026-11-18');
		expect(eleven.label).toContain('today');
	});
});

describe('the day link', () => {
	it('opens the Sessions page on that day, in the chosen project', () => {
		const [col] = stripColumns(timeSlots([], new Date(2026, 10, 18), NOW, false), [], false, NOW);
		expect(dayHref(col)).toBe(sessionsHref({ day: '2026-11-18' }));
		expect(dayHref(col, '/w/app')).toBe(sessionsHref({ day: '2026-11-18', project: '/w/app' }));
		expect(sessionsHref({ day: '2026-11-18' })).toMatch(/\/sessions\?day=2026-11-18$/);
	});
});

describe('standout: the day that stands out', () => {
	it('is the day farthest above the median active day, at 1.5 times it or more', () => {
		const days = [column('a', 10), column('b', 0), column('c', 10), column('d', 30)];
		expect(standout(days)).toEqual({ key: 'd', times: 3 });
	});

	it('is none when no day reaches 1.5 times the median', () => {
		expect(standout([column('a', 10), column('b', 14), column('c', 12)])).toBeNull();
		expect(standout([column('a', 7)])).toBeNull();
		expect(standout([column('a', 0)])).toBeNull();
	});

	it('says why in a line', () => {
		const days = [column('Mon', 10), column('Tue', 10), column('Wed', 30)];
		expect(standoutLine(days, standout(days), false)).toBe(
			'Wed stands out: 30 prompts, 3.0× a usual day.'
		);
		expect(standoutLine(days, null, true)).toBeNull();
	});
});

describe('median', () => {
	it('is the middle value, or the mean of the two middle ones', () => {
		expect(median([])).toBe(0);
		expect(median([3, 1, 2])).toBe(2);
		expect(median([4, 1, 2, 3])).toBe(2.5);
	});
});

describe('the hover readout', () => {
	const groups = [
		splitDay(
			'2026-11-18',
			[
				['/a', 1],
				['/b', 3]
			],
			{ cost_usd: 2.5, git_commits: 2 }
		)
	];
	const series = stripSeries(groups);
	const [today] = stripColumns(
		timeSlots(groups, new Date(2026, 10, 18), NOW, false),
		series,
		false,
		NOW
	);

	it('says prompts, spend, commits and the top project', () => {
		expect(stripReadout(today, series, true)).toEqual([
			'4 prompts',
			'$2.50 spent',
			'2 commits',
			'Mostly b'
		]);
	});

	it('leaves out spend no tool reports, and a top project for a quiet day', () => {
		const quiet = { ...today, value: 0, bucket: null, parts: [] };
		expect(stripReadout(quiet, series, false)).toEqual(['0 prompts', '0 commits']);
		expect(topSeries(quiet, series)).toBeNull();
	});
});

describe('streakLine', () => {
	it('says the active days and the streaks', () => {
		expect(streakLine({ active: 5, days: 7, longest: 3, current: 2 })).toBe(
			'Active on 5 days of 7; longest streak 3 days, current 2 days.'
		);
		expect(streakLine({ active: 1, days: 7, longest: 1, current: 0 })).toBe(
			'Active on 1 day of 7; longest streak 1 day.'
		);
	});
});
