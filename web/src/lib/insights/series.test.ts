import { describe, expect, it } from 'vitest';
import {
	bands,
	cacheShare,
	chartStart,
	hourKey,
	lengthUnit,
	metric,
	querySourceLabel,
	ratio,
	repeated,
	SIZES,
	slotWindow,
	streaks,
	timeSlots,
	weekGrid
} from '#lib/insights/series.ts';
import { bucket } from '#lib/testing/builders.ts';

const day = (key: string, prompts: number) => bucket({ key, prompts });

describe('timeSlots', () => {
	it('fills quiet days with empty slots, oldest first', () => {
		const slots = timeSlots(
			[day('2026-11-02', 3), day('2026-11-04', 1)],
			new Date(2026, 10, 2, 15),
			new Date(2026, 10, 4, 9),
			false
		);
		expect(slots.map((s) => [s.key, s.bucket?.prompts ?? 0])).toEqual([
			['2026-11-02', 3],
			['2026-11-03', 0],
			['2026-11-04', 1]
		]);
	});

	it('lays out clock hours with the day_hour keys', () => {
		const start = new Date(2026, 10, 7, 9, 30);
		const slots = timeSlots([day('2026-11-07 10', 2)], start, new Date(2026, 10, 7, 11, 5), true);
		expect(slots.map((s) => s.key)).toEqual(['2026-11-07 09', '2026-11-07 10', '2026-11-07 11']);
		expect(slots[1].bucket?.prompts).toBe(2);
		expect(hourKey(new Date(2026, 0, 3, 4))).toBe('2026-01-03 04');
	});

	it('gives a slot the window its prompts were sent in', () => {
		const [slot] = timeSlots([], new Date(2026, 10, 7), new Date(2026, 10, 7), false);
		const w = slotWindow(slot, false);
		expect(Date.parse(w.until) - Date.parse(w.since)).toBe(86_400_000);
		expect(new Date(w.since).getHours()).toBe(0);
	});

	it('starts all-time charts at the first prompt', () => {
		expect(chartStart(null, '2026-01-01T00:00:00Z', 0).toISOString()).toBe(
			'2026-01-01T00:00:00.000Z'
		);
		expect(chartStart(null, null, 5).getTime()).toBe(5);
	});
});

describe('streaks', () => {
	const slots = (counts: number[]) =>
		counts.map((n, i) => ({
			key: String(i),
			date: new Date(),
			bucket: n ? day(String(i), n) : null
		}));

	it('finds the longest run and the one going on now', () => {
		expect(streaks(slots([1, 1, 0, 1, 1, 1, 0, 2]))).toEqual({
			active: 6,
			days: 8,
			longest: 3,
			current: 1
		});
	});

	it('keeps a streak through yesterday while today is still empty', () => {
		expect(streaks(slots([1, 1, 1, 0])).current).toBe(3);
		expect(streaks(slots([1, 0, 0])).current).toBe(0);
		expect(streaks([])).toEqual({ active: 0, days: 0, longest: 0, current: 0 });
	});
});

describe('weekGrid', () => {
	it('puts ISO weekdays in the week_start order', () => {
		const groups = [bucket({ key: '7-23', prompts: 4 }), bucket({ key: '1-09', prompts: 2 })];
		const monday = weekGrid(groups, 'monday');
		expect(monday.rows[0].short).toBe('Mon');
		expect(monday.values[0][9]).toBe(2);
		expect(monday.values[6][23]).toBe(4);
		const sunday = weekGrid(groups, 'sunday');
		expect(sunday.rows[0].short).toBe('Sun');
		expect(sunday.values[0][23]).toBe(4);
		expect(sunday.isoDays[1]).toBe(1);
	});
});

describe('bands', () => {
	it('lists every band, empty ones included, with readable bounds', () => {
		const b = bands([bucket({ key: '00020', prompts: 5 })], SIZES, String);
		expect(b).toHaveLength(SIZES.length);
		expect(b[0]).toMatchObject({ key: '00000', label: 'Under 20', tick: '<20', bucket: null });
		expect(b[1]).toMatchObject({ key: '00020', label: '20 to 80', tick: '20' });
		expect(b[1].bucket?.prompts).toBe(5);
		expect(b.at(-1)).toMatchObject({ label: '1500 or more', tick: '1500+' });
	});

	it('writes session lengths compactly', () => {
		expect([0, 300, 3600, 14400].map(lengthUnit)).toEqual(['0s', '5m', '1h', '4h']);
	});
});

describe('ratios', () => {
	it('is null, not zero, when there is nothing to divide by', () => {
		expect(ratio(1, 0)).toBeNull();
		expect(ratio(1, 4)).toBe(0.25);
	});

	it('shares prompt tokens served from cache', () => {
		expect(
			cacheShare(bucket({ input_tokens: 10, cache_read_tokens: 30, cache_creation_tokens: 10 }))
		).toBe(0.6);
		expect(cacheShare(bucket())).toBeNull();
	});

	it('counts prompts that repeat an earlier text', () => {
		expect(repeated(bucket({ prompts: 10, distinct_texts: 7 }))).toBe(3);
	});
});

describe('telemetry and query sources', () => {
	it('reads a metric, null when it never arrived', () => {
		const t = {
			sessions: 1,
			metrics: { 'claude_code.lines_of_code.count[added]': 12 },
			commands: []
		};
		expect(metric(t, 'linesAdded')).toBe(12);
		expect(metric(t, 'commits')).toBeNull();
	});

	it('names query sources', () => {
		expect(querySourceLabel('agent:builtin:Explore')).toBe('Explore');
		expect(querySourceLabel('generate_session_title')).toBe('Generate session title');
		expect(querySourceLabel(null)).toBe('Main thread');
	});
});
