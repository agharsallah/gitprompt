import { describe, expect, it } from 'vitest';
import {
	LONG_TURN_MINUTES,
	minuteOptions,
	minutesLabel,
	switchOf
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
