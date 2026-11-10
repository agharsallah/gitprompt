import { describe, expect, it } from 'vitest';
import {
	DEFAULT_RANGE,
	parseRange,
	rangePhrase,
	rangeStart,
	windowStart
} from '#lib/insights/range.ts';

describe('ranges', () => {
	it('reads a known range from the URL and falls back to the default', () => {
		expect(parseRange('30d').id).toBe('30d');
		expect(parseRange('nonsense').id).toBe(DEFAULT_RANGE);
		expect(parseRange(null).id).toBe(DEFAULT_RANGE);
		expect(parseRange(null, '30d').id).toBe('30d');
		expect(parseRange('24h', '30d').id).toBe('24h');
	});

	it('starts a window that many days back, or never for all time', () => {
		const now = Date.parse('2026-11-07T12:00:00Z');
		expect(rangeStart(parseRange('7d'), now)?.toISOString()).toBe('2026-10-31T12:00:00.000Z');
		expect(rangeStart(parseRange('all'), now)).toBeNull();
	});

	it('reads as part of a sentence', () => {
		expect(rangePhrase(parseRange('24h'))).toBe('in the last 24 hours');
		expect(rangePhrase(parseRange('all'))).toBe('so far');
	});

	it('starts a window at the later of its range and a start of its own, so both hold', () => {
		const now = Date.parse('2026-11-07T12:00:00Z');
		const week = parseRange('7d');
		expect(windowStart(week, undefined, now)).toBe('2026-10-31T12:00:00.000Z');
		expect(windowStart(week, '2026-11-05T00:00:00Z', now)).toBe('2026-11-05T00:00:00Z');
		expect(windowStart(week, '2026-01-01T00:00:00Z', now)).toBe('2026-10-31T12:00:00.000Z');
		expect(windowStart(parseRange('all'), '2026-01-01T00:00:00Z', now)).toBe(
			'2026-01-01T00:00:00Z'
		);
		expect(windowStart(parseRange('all'), undefined, now)).toBeUndefined();
	});
});
