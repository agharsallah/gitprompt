/**
 * The choices of the Notifications settings: on or off per kind, a threshold picked among a few
 * durations, and the weekly recap's local day and hour. The daemon accepts any whole number of minutes (1 to 1440); a value set in the
 * file or an environment variable that is not a preset is offered too, so the control shows it.
 */
import type { Weekday } from '#lib/api/types.ts';
import { plural } from '#lib/shared/format.ts';

export const SWITCH = [
	{ value: 'on' as const, label: 'On' },
	{ value: 'off' as const, label: 'Off' }
];

export type Switch = (typeof SWITCH)[number]['value'];

export const switchOf = (on: boolean): Switch => (on ? 'on' : 'off');

/** How long a turn runs before its end is worth a notification. */
export const LONG_TURN_MINUTES = [5, 10, 20, 30, 60];
/** How long a connector is silent before it counts as quiet. */
export const QUIET_MINUTES = [15, 30, 60, 120];

/** `90` → `1 h 30 min`, `10` → `10 min`. */
export function minutesLabel(minutes: number): string {
	const hours = Math.floor(minutes / 60);
	const rest = minutes % 60;
	if (!hours) return `${minutes} min`;
	return rest ? `${hours} h ${rest} min` : plural(hours, 'hour');
}

/** The presets, plus `current` when it is not one, in order, as segmented-control options. */
export function minuteOptions(presets: number[], current: number) {
	const all = presets.includes(current) ? presets : [...presets, current].sort((a, b) => a - b);
	return all.map((minutes) => ({ value: String(minutes), label: minutesLabel(minutes) }));
}

/** The days the weekly recap can come on, Monday first. */
export const WEEKDAYS: { value: Weekday; label: string }[] = [
	{ value: 'monday', label: 'Monday' },
	{ value: 'tuesday', label: 'Tuesday' },
	{ value: 'wednesday', label: 'Wednesday' },
	{ value: 'thursday', label: 'Thursday' },
	{ value: 'friday', label: 'Friday' },
	{ value: 'saturday', label: 'Saturday' },
	{ value: 'sunday', label: 'Sunday' }
];

/** `monday` → `Monday`. */
export const weekdayLabel = (day: Weekday) => WEEKDAYS.find((d) => d.value === day)?.label ?? day;

/** The day picked in a select, if it is one. */
export const weekdayOf = (value: string): Weekday | undefined =>
	WEEKDAYS.find((d) => d.value === value)?.value;

/** `9` → `09:00` (local time). */
export const hourLabel = (hour: number) => `${String(hour).padStart(2, '0')}:00`;

/** Every hour of the day, as select options. */
export const HOURS = Array.from({ length: 24 }, (_, hour) => ({
	value: String(hour),
	label: hourLabel(hour)
}));
