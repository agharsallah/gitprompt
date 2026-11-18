/**
 * The choices of the Notifications settings: on or off per kind, and a threshold picked among a
 * few durations. The daemon accepts any whole number of minutes (1 to 1440); a value set in the
 * file or an environment variable that is not a preset is offered too, so the control shows it.
 */
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
