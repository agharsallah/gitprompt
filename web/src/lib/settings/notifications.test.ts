import { describe, expect, it } from 'vitest';
import {
	HOURS,
	hourLabel,
	LONG_TURN_MINUTES,
	minuteOptions,
	minutesLabel,
	switchOf,
	WEEKDAYS,
	weekdayLabel,
	weekdayOf
} from '#lib/settings/notifications.ts';

describe('minutesLabel', () => {
	it('says minutes, hours, or both', () => {
		expect([5, 60, 90, 120].map(minutesLabel)).toEqual([
			'5 min',
			'1 hour',
			'1 h 30 min',
			'2 hours'
		]);
	});
});

describe('minuteOptions', () => {
	it('offers the presets', () => {
		expect(minuteOptions(LONG_TURN_MINUTES, 10).map((o) => o.value)).toEqual([
			'5',
			'10',
			'20',
			'30',
			'60'
		]);
	});

	it('adds a value set elsewhere in its place, so the control shows it', () => {
		expect(minuteOptions([5, 10, 20], 15).map((o) => [o.value, o.label])).toEqual([
			['5', '5 min'],
			['10', '10 min'],
			['15', '15 min'],
			['20', '20 min']
		]);
	});
});

describe('switchOf', () => {
	it('names a switch', () => {
		expect([switchOf(true), switchOf(false)]).toEqual(['on', 'off']);
	});
});

describe('the weekly recap day and hour', () => {
	it('offers every day, Monday first, and reads a picked one back', () => {
		expect(WEEKDAYS.map((d) => d.value)).toEqual([
			'monday',
			'tuesday',
			'wednesday',
			'thursday',
			'friday',
			'saturday',
			'sunday'
		]);
		expect(weekdayLabel('friday')).toBe('Friday');
		expect(weekdayOf('sunday')).toBe('sunday');
		expect(weekdayOf('someday')).toBeUndefined();
	});

	it('offers every local hour, written as a time', () => {
		expect(HOURS).toHaveLength(24);
		expect([HOURS[0], HOURS[23]]).toEqual([
			{ value: '0', label: '00:00' },
			{ value: '23', label: '23:00' }
		]);
		expect(hourLabel(9)).toBe('09:00');
	});
});
