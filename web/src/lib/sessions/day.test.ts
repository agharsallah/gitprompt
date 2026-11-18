import { describe, expect, it } from 'vitest';
import { dayPhrase, dayWindow } from '#lib/sessions/day.ts';

describe('dayWindow', () => {
	it('spans a local day, midnight to midnight', () => {
		expect(dayWindow('2026-11-05')).toEqual({
			since: new Date(2026, 10, 5).toISOString(),
			until: new Date(2026, 10, 6).toISOString()
		});
	});

	it.each([null, undefined, '', 'yesterday', '2026-02-30', '2026-1-5'])(
		'reads %s as no day',
		(day) => expect(dayWindow(day)).toBeNull()
	);
});

describe('dayPhrase', () => {
	const now = new Date(2026, 10, 18, 12);
	it('names today and yesterday, and any other day by its date', () => {
		expect(dayPhrase('2026-11-18', now)).toBe('today');
		expect(dayPhrase('2026-11-17', now)).toBe('yesterday');
		expect(dayPhrase('2026-11-05', now)).toBe('on Thursday 5 November');
	});
});
