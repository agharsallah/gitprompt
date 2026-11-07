/** Which day a week starts on (the `week_start` setting) and the weekday rows in that order. */
export type WeekStart = 'monday' | 'sunday';

const MONDAY_FIRST = [
	{ short: 'Mon', plural: 'Mondays' },
	{ short: 'Tue', plural: 'Tuesdays' },
	{ short: 'Wed', plural: 'Wednesdays' },
	{ short: 'Thu', plural: 'Thursdays' },
	{ short: 'Fri', plural: 'Fridays' },
	{ short: 'Sat', plural: 'Saturdays' },
	{ short: 'Sun', plural: 'Sundays' }
] as const;

export type Weekday = (typeof MONDAY_FIRST)[number];

/** The seven weekdays, starting with `start`. */
export function weekdays(start: WeekStart = 'monday'): Weekday[] {
	return start === 'sunday' ? [MONDAY_FIRST[6], ...MONDAY_FIRST.slice(0, 6)] : [...MONDAY_FIRST];
}

/** The row of a local date in a week that starts on `start` (0 = first day). */
export function weekRow(date: Date, start: WeekStart = 'monday'): number {
	return start === 'sunday' ? date.getDay() : (date.getDay() + 6) % 7;
}
