import { describe, expect, it } from 'vitest';
import { weekRow, weekdays } from '#lib/insights/week.ts';

describe('week start', () => {
	it('orders weekdays from Monday or from Sunday', () => {
		expect(weekdays().map((d) => d.short)).toEqual([
			'Mon',
			'Tue',
			'Wed',
			'Thu',
			'Fri',
			'Sat',
			'Sun'
		]);
		expect(weekdays('sunday').map((d) => d.short)).toEqual([
			'Sun',
			'Mon',
			'Tue',
			'Wed',
			'Thu',
			'Fri',
			'Sat'
		]);
	});

	it('puts a local date on the matching row', () => {
		const sunday = new Date(2026, 10, 8, 12);
		const thursday = new Date(2026, 10, 5, 12);
		expect(weekRow(sunday)).toBe(6);
		expect(weekRow(sunday, 'sunday')).toBe(0);
		expect(weekRow(thursday)).toBe(3);
		expect(weekRow(thursday, 'sunday')).toBe(4);
		// The row and its label always agree.
		for (const start of ['monday', 'sunday'] as const) {
			expect(weekdays(start)[weekRow(thursday, start)].short).toBe('Thu');
		}
	});
});
